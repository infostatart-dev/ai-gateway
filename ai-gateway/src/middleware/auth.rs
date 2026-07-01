use axum_core::response::IntoResponse;
use futures::future::BoxFuture;
use http::{Method, Request};
use tower_http::auth::AsyncAuthorizeRequest;

use crate::{
    app_state::AppState,
    client_access::ClientAccessKeyHash,
    control_plane::types::hash_key,
    error::{
        api::ApiError, auth::AuthError, internal::InternalError,
        invalid_req::InvalidRequestError,
    },
    router::router_details::RouteType,
    types::{
        extensions::{AuthContext, ClientAccessContext, RequestKind},
        router::RouterId,
        secret::Secret,
    },
};

#[derive(Clone)]
pub struct AuthService {
    app_state: AppState,
}

impl AuthService {
    #[must_use]
    pub fn new(app_state: AppState) -> Self {
        Self { app_state }
    }

    fn is_public_request<B>(request: &Request<B>) -> bool {
        if !matches!(request.method(), &Method::GET | &Method::HEAD) {
            return false;
        }
        let path = request.uri().path();
        path == "/health"
            || path == "/v1/observability/provider-stats"
            || path.starts_with("/v1/observability/provider-stats/")
    }

    async fn authenticate_request_inner(
        app_state: AppState,
        api_key: &str,
        request_kind: Option<&RequestKind>,
        router_id: Option<&RouterId>,
    ) -> Result<AuthContext, ApiError> {
        let api_key_without_bearer = api_key.replace("Bearer ", "");
        let computed_hash = hash_key(&api_key_without_bearer);

        if app_state.0.config.deployment_target.is_cloud() {
            let Some(request_kind) = request_kind else {
                return Err(
                    InternalError::ExtensionNotFound("RequestKind").into()
                );
            };
            let Some(key) =
                app_state.check_helicone_api_key(&computed_hash).await
            else {
                return Err(AuthError::InvalidCredentials.into());
            };

            match request_kind {
                RequestKind::Router => {
                    let Some(router_id) = router_id else {
                        return Err(InternalError::ExtensionNotFound(
                            "RouterId",
                        )
                        .into());
                    };

                    let Some(router_organization_id) =
                        app_state.get_router_organization(router_id).await
                    else {
                        return Err(InvalidRequestError::NotFound(
                            "router not found".to_string(),
                        )
                        .into());
                    };

                    if router_organization_id == key.organization_id {
                        Ok(AuthContext {
                            api_key: Secret::from(api_key_without_bearer),
                            user_id: key.owner_id,
                            org_id: key.organization_id,
                        })
                    } else {
                        Err(AuthError::InvalidCredentials.into())
                    }
                }
                RequestKind::UnifiedApi | RequestKind::DirectProxy => {
                    Ok(AuthContext {
                        api_key: Secret::from(api_key_without_bearer),
                        user_id: key.owner_id,
                        org_id: key.organization_id,
                    })
                }
            }
        } else {
            let Some(control_plane_state) =
                &app_state.0.control_plane_state.read().await.state
            else {
                return Err(InternalError::AuthDataNotReady.into());
            };
            let key = control_plane_state.get_key_from_hash(&computed_hash);
            if let Some(key) = key {
                Ok(AuthContext {
                    api_key: Secret::from(api_key_without_bearer),
                    user_id: key.owner_id,
                    org_id: control_plane_state.auth.organization_id,
                })
            } else {
                Err(AuthError::InvalidCredentials.into())
            }
        }
    }

    fn authenticate_client_access(
        app_state: &AppState,
        bearer_token: &str,
        request_kind: Option<&RequestKind>,
        router_id: Option<&RouterId>,
        route_type: Option<&RouteType>,
    ) -> Result<(ClientAccessContext, AuthContext), ApiError> {
        let Some(request_kind) = request_kind else {
            return Err(InternalError::ExtensionNotFound("RequestKind").into());
        };
        let snapshot = app_state
            .client_access_snapshot()
            .ok_or(InternalError::AuthDataNotReady)?;
        let key_hash = ClientAccessKeyHash::from_bearer_token(bearer_token);
        let key = snapshot
            .lookup_hash(&key_hash)
            .ok_or(AuthError::InvalidCredentials)?;
        if !key.is_active_at(chrono::Utc::now()) {
            return Err(AuthError::InvalidCredentials.into());
        }

        let direct_provider = match route_type {
            Some(RouteType::DirectProxy { provider, .. }) => Some(provider),
            _ => None,
        };
        if !key.allows(request_kind, router_id, direct_provider) {
            return Err(AuthError::ScopeDenied.into());
        }

        let auth_context = AuthContext {
            api_key: Secret::from(bearer_token.to_string()),
            user_id: key.subject.user_id,
            org_id: key.subject.org_id,
        };
        let client_access_context = ClientAccessContext {
            key_id: key.id.to_string(),
            subject_id: key.subject.id.to_string(),
            user_id: key.subject.user_id,
            org_id: key.subject.org_id,
            plan_id: key.plan.id.to_string(),
            max_output_tokens: key.plan.max_output_tokens,
            scopes: key.scopes.iter().cloned().collect(),
            quota_limits: key.plan.limits.clone(),
        };
        Ok((client_access_context, auth_context))
    }
}

impl<B> AsyncAuthorizeRequest<B> for AuthService
where
    B: Send + 'static,
{
    type RequestBody = B;
    type ResponseBody = axum_core::body::Body;
    type Future = BoxFuture<
        'static,
        Result<Request<B>, http::Response<Self::ResponseBody>>,
    >;

    #[tracing::instrument(skip_all)]
    fn authorize(&mut self, mut request: Request<B>) -> Self::Future {
        let app_state = self.app_state.clone();
        Box::pin(async move {
            if Self::is_public_request(&request) {
                tracing::trace!("auth middleware: public endpoint");
                return Ok(request);
            }
            if !app_state.0.config.client_access.enabled
                && app_state.0.config.helicone.is_auth_disabled()
            {
                tracing::trace!("auth middleware: auth disabled");
                return Ok(request);
            }
            tracing::trace!("auth middleware");
            let Some(authorization) = request
                .headers()
                .get("authorization")
                .and_then(|h| h.to_str().ok())
            else {
                return Err(
                    AuthError::MissingAuthorizationHeader.into_response()
                );
            };
            app_state.0.metrics.auth_attempts.add(1, &[]);

            let request_kind = request.extensions().get::<RequestKind>();
            let router_id = request.extensions().get::<RouterId>();
            let route_type = request.extensions().get::<RouteType>();
            if app_state.0.config.client_access.enabled {
                app_state.0.metrics.client_access.auth_attempts.add(1, &[]);
                let Some(bearer_token) = authorization.strip_prefix("Bearer ")
                else {
                    app_state.0.metrics.auth_rejections.add(1, &[]);
                    app_state
                        .0
                        .metrics
                        .client_access
                        .auth_rejections
                        .add(1, &[]);
                    return Err(AuthError::InvalidCredentials.into_response());
                };
                match Self::authenticate_client_access(
                    &app_state,
                    bearer_token,
                    request_kind,
                    router_id,
                    route_type,
                ) {
                    Ok((client_access_ctx, auth_ctx)) => {
                        request.extensions_mut().insert(client_access_ctx);
                        request.extensions_mut().insert(auth_ctx);
                        return Ok(request);
                    }
                    Err(e) => {
                        if let ApiError::Authentication(auth_error) = &e {
                            app_state.0.metrics.auth_rejections.add(1, &[]);
                            app_state
                                .0
                                .metrics
                                .client_access
                                .auth_rejections
                                .add(1, &[]);
                            if matches!(auth_error, AuthError::ScopeDenied) {
                                app_state
                                    .0
                                    .metrics
                                    .client_access
                                    .scope_denials
                                    .add(1, &[]);
                            }
                        }
                        return Err(e.into_response());
                    }
                }
            }

            match Self::authenticate_request_inner(
                app_state.clone(),
                authorization,
                request_kind,
                router_id,
            )
            .await
            {
                Ok(auth_ctx) => {
                    request.extensions_mut().insert(auth_ctx);
                    Ok(request)
                }
                Err(e) => {
                    if let ApiError::Authentication(auth_error) = &e {
                        match auth_error {
                            AuthError::MissingAuthorizationHeader
                            | AuthError::InvalidCredentials
                            | AuthError::ScopeDenied
                            | AuthError::ProviderKeyNotFound => {
                                app_state.0.metrics.auth_rejections.add(1, &[]);
                            }
                        }
                    }
                    Err(e.into_response())
                }
            }
        })
    }
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use std::path::PathBuf;

    use axum_core::body::Body;
    use http::StatusCode;
    use tower_http::auth::AsyncAuthorizeRequest;
    use uuid::Uuid;

    use super::*;
    use crate::{
        app::App,
        client_access::ClientAccessKeyHash,
        config::{
            Config,
            client_access::{ClientAccessConfig, ClientAccessQuotaStoreConfig},
        },
        control_plane::types::ControlPlaneState,
        router::router_details::RouteType,
        tests::TestDefault,
        types::{provider::InferenceProvider, router::RouterId},
    };

    const CLIENT_TOKEN: &str = "sk-client-test";

    fn temp_registry_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "ai-gateway-client-access-auth-{}-{}.yaml",
            std::process::id(),
            Uuid::new_v4()
        ))
    }

    fn registry_yaml(
        key_id: &str,
        token: &str,
        scopes: &[&str],
        status: &str,
        expires_at: Option<&str>,
    ) -> String {
        let hash = ClientAccessKeyHash::from_bearer_token(token);
        let scopes = scopes
            .iter()
            .map(|scope| format!("      - \"{scope}\"\n"))
            .collect::<String>();
        let expires = expires_at
            .map(|value| format!("    expires-at: \"{value}\"\n"))
            .unwrap_or_default();
        format!(
            r#"
version: 1
subjects:
  acme:
    org-id: "00000000-0000-0000-0000-000000000001"
    user-id: "00000000-0000-0000-0000-000000000002"
plans:
  starter:
    limits:
      requests:
        per-minute: 10
      tokens:
        per-minute: 1000
keys:
  {key_id}:
    hash: "{hash}"
    subject: acme
    status: {status}
    plan: starter
{expires}    scopes:
{scopes}"#
        )
    }

    async fn app_state_for_registry(yaml: String) -> AppState {
        let path = temp_registry_path();
        std::fs::write(&path, yaml).unwrap();
        let mut config = Config::test_default();
        config.client_access = ClientAccessConfig {
            enabled: true,
            file: Some(path),
            reload_interval: std::time::Duration::from_secs(1),
            max_body_bytes: 1024,
            quota_store: ClientAccessQuotaStoreConfig::Memory,
        };
        App::new(config).await.unwrap().state
    }

    fn protected_request(
        auth: Option<&str>,
        kind: RequestKind,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(Method::POST)
            .uri("http://gateway.local/ai/chat/completions");
        if let Some(auth) = auth {
            builder = builder.header("authorization", auth);
        }
        let mut request = builder.body(Body::empty()).unwrap();
        request.extensions_mut().insert(kind);
        request
    }

    async fn authorize(
        app_state: AppState,
        request: Request<Body>,
    ) -> Result<Request<Body>, http::Response<Body>> {
        AuthService::new(app_state).authorize(request).await
    }

    #[tokio::test]
    async fn client_access_auth_rejects_missing_invalid_suspended_and_expired_keys()
     {
        let app_state = app_state_for_registry(registry_yaml(
            "client-key",
            CLIENT_TOKEN,
            &["unified-api"],
            "active",
            None,
        ))
        .await;

        let missing = authorize(
            app_state.clone(),
            protected_request(None, RequestKind::UnifiedApi),
        )
        .await
        .unwrap_err();
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let invalid = authorize(
            app_state.clone(),
            protected_request(
                Some("Bearer sk-unknown"),
                RequestKind::UnifiedApi,
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);

        let suspended_state = app_state_for_registry(registry_yaml(
            "client-key",
            CLIENT_TOKEN,
            &["unified-api"],
            "suspended",
            None,
        ))
        .await;
        let suspended = authorize(
            suspended_state,
            protected_request(
                Some("Bearer sk-client-test"),
                RequestKind::UnifiedApi,
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(suspended.status(), StatusCode::UNAUTHORIZED);

        let expired_state = app_state_for_registry(registry_yaml(
            "client-key",
            CLIENT_TOKEN,
            &["unified-api"],
            "active",
            Some("2020-01-01T00:00:00Z"),
        ))
        .await;
        let expired = authorize(
            expired_state,
            protected_request(
                Some("Bearer sk-client-test"),
                RequestKind::UnifiedApi,
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(expired.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn client_access_auth_attaches_context_for_active_key() {
        let app_state = app_state_for_registry(registry_yaml(
            "client-key",
            CLIENT_TOKEN,
            &["unified-api"],
            "active",
            None,
        ))
        .await;

        let request = authorize(
            app_state,
            protected_request(
                Some("Bearer sk-client-test"),
                RequestKind::UnifiedApi,
            ),
        )
        .await
        .unwrap();

        let client_ctx = request
            .extensions()
            .get::<ClientAccessContext>()
            .expect("client access context");
        assert_eq!(client_ctx.key_id, "client-key");
        assert_eq!(client_ctx.plan_id, "starter");
        assert!(request.extensions().get::<AuthContext>().is_some());
    }

    #[tokio::test]
    async fn client_access_scopes_cover_unified_router_direct_and_wildcard() {
        let scoped_state = app_state_for_registry(registry_yaml(
            "client-key",
            CLIENT_TOKEN,
            &["unified-api", "router:private", "direct:openrouter"],
            "active",
            None,
        ))
        .await;

        authorize(
            scoped_state.clone(),
            protected_request(
                Some("Bearer sk-client-test"),
                RequestKind::UnifiedApi,
            ),
        )
        .await
        .unwrap();

        let mut router_req = protected_request(
            Some("Bearer sk-client-test"),
            RequestKind::Router,
        );
        router_req
            .extensions_mut()
            .insert(RouterId::Named("private".into()));
        authorize(scoped_state.clone(), router_req).await.unwrap();

        let mut direct_req = protected_request(
            Some("Bearer sk-client-test"),
            RequestKind::DirectProxy,
        );
        direct_req.extensions_mut().insert(RouteType::DirectProxy {
            provider: InferenceProvider::OpenRouter,
            path: "chat/completions".into(),
        });
        authorize(scoped_state, direct_req).await.unwrap();

        let wildcard_state = app_state_for_registry(registry_yaml(
            "client-key",
            CLIENT_TOKEN,
            &["*"],
            "active",
            None,
        ))
        .await;
        let mut wildcard_req = protected_request(
            Some("Bearer sk-client-test"),
            RequestKind::DirectProxy,
        );
        wildcard_req
            .extensions_mut()
            .insert(RouteType::DirectProxy {
                provider: InferenceProvider::OpenAI,
                path: "chat/completions".into(),
            });
        authorize(wildcard_state, wildcard_req).await.unwrap();
    }

    #[tokio::test]
    async fn client_access_scope_denial_returns_forbidden() {
        let app_state = app_state_for_registry(registry_yaml(
            "client-key",
            CLIENT_TOKEN,
            &["router:default"],
            "active",
            None,
        ))
        .await;
        let mut request = protected_request(
            Some("Bearer sk-client-test"),
            RequestKind::Router,
        );
        request
            .extensions_mut()
            .insert(RouterId::Named("private".into()));

        let response = authorize(app_state, request).await.unwrap_err();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn legacy_auth_fallback_still_works_when_client_access_disabled() {
        let app_state = App::new(Config::test_default()).await.unwrap().state;
        app_state.0.control_plane_state.write().await.state =
            Some(ControlPlaneState::test_default());

        let request = authorize(
            app_state,
            protected_request(
                Some("Bearer sk-helicone-test-key"),
                RequestKind::UnifiedApi,
            ),
        )
        .await
        .unwrap();

        assert!(request.extensions().get::<AuthContext>().is_some());
        assert!(request.extensions().get::<ClientAccessContext>().is_none());
    }
}

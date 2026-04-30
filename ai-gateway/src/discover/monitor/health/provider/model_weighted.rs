use super::inner::ProviderMonitorInner;
use crate::{
    config::balance::BalanceConfigInner,
    discover::model::weighted_key::WeightedKey as ModelWeightedKey,
    dispatcher::Dispatcher,
    error::{init::InitError, internal::InternalError, runtime::RuntimeError},
};
use opentelemetry::KeyValue;
use rust_decimal::prelude::ToPrimitive;
use tokio::task::JoinSet;
use tower::discover::Change;
use tracing::{error, trace};
use weighted_balance::weight::Weight;

#[allow(clippy::too_many_lines)]
pub async fn check_model_weighted_monitor(
    inner: &mut ProviderMonitorInner<ModelWeightedKey>,
) -> Result<(), RuntimeError> {
    for (endpoint_type, balance_config) in
        inner.router_config.load_balance.as_ref()
    {
        match balance_config {
            BalanceConfigInner::ModelWeighted { models } => {
                for target in models {
                    let model = &target.model;
                    let provider =
                        model.inference_provider().ok_or_else(|| {
                            InitError::ModelIdNotRecognized(model.to_string())
                        })?;
                    let weight =
                        Weight::from(target.weight.to_f64().ok_or_else(
                            || InitError::InvalidWeight(provider.clone()),
                        )?);
                    let key = ModelWeightedKey::new(
                        model.clone(),
                        *endpoint_type,
                        weight,
                    );
                    let is_healthy = inner.check_health(&provider)?;
                    let was_unhealthy = inner.unhealthy_keys.contains(&key);

                    if !is_healthy && !was_unhealthy {
                        trace!(provider = ?provider, endpoint_type = ?endpoint_type, "Provider became unhealthy, removing");
                        let all_models = models
                            .iter()
                            .filter(|m| {
                                m.model.inference_provider().as_ref()
                                    == Some(&provider)
                            })
                            .collect::<Vec<_>>();
                        let mut join_set = JoinSet::new();
                        for unhealthy_model in all_models {
                            let weight = Weight::from(
                                unhealthy_model.weight.to_f64().ok_or_else(
                                    || {
                                        InitError::InvalidWeight(
                                            provider.clone(),
                                        )
                                    },
                                )?,
                            );
                            let unhealthy_key = ModelWeightedKey::new(
                                unhealthy_model.model.clone(),
                                *endpoint_type,
                                weight,
                            );
                            let tx = inner.tx.clone();
                            inner.unhealthy_keys.insert(unhealthy_key.clone());
                            join_set.spawn(async move {
                                tx.send(Change::Remove(unhealthy_key)).await
                            });
                        }
                        while let Some(task_result) = join_set.join_next().await
                        {
                            match task_result {
                                Ok(send_result) => {
                                    if let Err(e) = send_result {
                                        error!(error = ?e, model = ?model, "Failed to send remove event for unhealthy provider model");
                                    }
                                }
                                Err(e) => {
                                    error!(error = ?e, "Task failed while sending remove event for unhealthy provider model");
                                    return Err(e.into());
                                }
                            }
                        }
                    } else if is_healthy && was_unhealthy {
                        trace!(provider = ?provider, endpoint_type = ?endpoint_type, "Provider became healthy, adding back");
                        let all_models = models
                            .iter()
                            .filter(|m| {
                                m.model.inference_provider().as_ref()
                                    == Some(&provider)
                            })
                            .collect::<Vec<_>>();
                        inner.unhealthy_keys.remove(&key);
                        for healthy_model in all_models {
                            let weight = Weight::from(
                                healthy_model.weight.to_f64().ok_or_else(
                                    || {
                                        InitError::InvalidWeight(
                                            provider.clone(),
                                        )
                                    },
                                )?,
                            );
                            let key = ModelWeightedKey::new(
                                healthy_model.model.clone(),
                                *endpoint_type,
                                weight,
                            );
                            let service = Dispatcher::new(
                                inner.app_state.clone(),
                                &inner.router_id,
                                &inner.router_config,
                                provider.clone(),
                            )
                            .await?;
                            if let Err(e) = inner
                                .tx
                                .send(Change::Insert(key, service))
                                .await
                            {
                                error!(error = ?e, "Failed to send insert event for healthy provider");
                            }
                        }
                    }
                    inner.app_state.0.metrics.provider_health.record(
                        if is_healthy { 1 } else { 0 },
                        &[KeyValue::new("provider", provider.to_string())],
                    );
                }
            }
            _ => return Err(InternalError::Internal.into()),
        }
    }
    Ok(())
}

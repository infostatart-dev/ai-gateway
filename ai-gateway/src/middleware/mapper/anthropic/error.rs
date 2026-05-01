use http::response::Parts;

use crate::{
    endpoints::anthropic::messages::AnthropicApiError,
    error::mapper::MapperError, middleware::mapper::TryConvertError,
};

impl TryConvertError<AnthropicApiError, async_openai::error::WrappedError>
    for super::AnthropicConverter
{
    type Error = MapperError;
    fn try_convert_error(
        &self,
        resp_parts: &Parts,
        value: AnthropicApiError,
    ) -> Result<async_openai::error::WrappedError, Self::Error> {
        let message = value.error.message;
        let error = crate::middleware::mapper::openai_error_from_status(
            resp_parts.status,
            Some(message),
        );
        Ok(error)
    }
}

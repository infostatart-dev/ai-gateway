use http::response::Parts;
use crate::{
    error::mapper::MapperError,
    middleware::mapper::TryConvertError,
    endpoints::bedrock::converse::ConverseError,
};

impl TryConvertError<ConverseError, async_openai::error::WrappedError> for super::BedrockConverter {
    type Error = MapperError;
    fn try_convert_error(&self, resp_parts: &Parts, _value: ConverseError) -> Result<async_openai::error::WrappedError, Self::Error> {
        Ok(crate::middleware::mapper::openai_error_from_status(resp_parts.status, None))
    }
}

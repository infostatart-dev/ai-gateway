use http::response::Parts as ResponseParts;
use crate::{
    error::mapper::MapperError,
    middleware::mapper::TryConvertError,
};

impl TryConvertError<async_openai::error::WrappedError, async_openai::error::WrappedError> for super::OpenAIConverter {
    type Error = MapperError;
    fn try_convert_error(&self, _resp_parts: &ResponseParts, value: async_openai::error::WrappedError) -> Result<async_openai::error::WrappedError, Self::Error> {
        Ok(value)
    }
}

//! Consistent classification of production envd HTTP failures.

use crate::error::Error;

pub(super) fn request_error(
    source: reqwest::Error,
    ambiguous: bool,
    context: &'static str,
) -> Error {
    if ambiguous {
        return Error::DeliveryAmbiguous;
    }
    if source.is_timeout()
        || source.is_connect()
        || source.is_body()
        || source.is_decode()
        || source.is_request()
    {
        return Error::Unavailable;
    }
    Error::internal_with(source, context)
}

#[cfg(test)]
#[path = "_tests_/http_error_tests.rs"]
mod http_error_tests;

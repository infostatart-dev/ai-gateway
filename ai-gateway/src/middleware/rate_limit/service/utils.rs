pub fn increment_retry_after_header<ResponseBody>(res: &mut http::Response<ResponseBody>) {
    if let Some(retry_after) = res.headers().get("retry-after") {
        if let Some(val) = retry_after.to_str().ok().and_then(|s| s.parse::<u64>().ok()) {
            let new_val = val + 1;
            res.headers_mut().insert("retry-after", new_val.to_string().parse().unwrap());
            res.headers_mut().insert("x-ratelimit-after", new_val.to_string().parse().unwrap());
        }
    }
}

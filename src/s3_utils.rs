// Helpers for working with public S3-compatible URLs.
pub fn build_public_url(base: &str, bucket: &str, key: &str) -> String {
    let trimmed = base.trim_end_matches('/');

    // Allow simple templating: https://host/{bucket}/{key} or https://bucket.host/{key}
    if trimmed.contains("{bucket}") || trimmed.contains("{key}") {
        return trimmed.replace("{bucket}", bucket).replace("{key}", key);
    }

    // If the base already includes the bucket, append only the key.
    if trimmed.contains(bucket) {
        format!("{}/{}", trimmed, key)
    } else {
        format!("{}/{}/{}", trimmed, bucket, key)
    }
}

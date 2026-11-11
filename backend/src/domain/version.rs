//! Version numbers of the releases (`X.Y.Z`, optionally prefixed with `v`).

fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let version = version.trim().trim_start_matches('v');
    // `2.1.0-rc1` / `2.1.0+build`: only the numeric core is compared.
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let numbers = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(numbers)
}

/// Whether `latest` is a strictly newer version than `current`. Anything that is not
/// an `X.Y.Z` version is never considered newer.
pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(l), Some(c)) if l > c)
}

/// `v2.1.0` → `2.1.0`.
pub fn without_prefix(tag: &str) -> &str {
    tag.trim().trim_start_matches('v')
}

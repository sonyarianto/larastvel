//! DNS lookup faking for DNS-backed validation rules.
//!
//! Mirrors Laravel 13.22's `Validator::fakeDnsLookups()`: DNS-backed rules
//! such as [`active_url`](crate::validation::active_url) normally perform a
//! real lookup; faking skips only the network call while preserving the rest
//! of the rule's behavior (malformed values still fail).

use std::sync::atomic::{AtomicBool, Ordering};

static DNS_LOOKUPS_FAKED: AtomicBool = AtomicBool::new(false);

/// Fake (or stop faking) DNS lookups performed by DNS-backed validation
/// rules. Returns the previous state so callers can restore it.
///
/// ```rust,ignore
/// // In a test:
/// fake_dns_lookups(true);
/// // ... assert active_url() passes for any well-formed URL ...
/// fake_dns_lookups(false);
/// ```
pub fn fake_dns_lookups(fake: bool) -> bool {
    DNS_LOOKUPS_FAKED.swap(fake, Ordering::SeqCst)
}

pub(crate) fn dns_lookups_faked() -> bool {
    DNS_LOOKUPS_FAKED.load(Ordering::SeqCst)
}

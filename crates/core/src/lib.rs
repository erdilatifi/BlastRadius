mod github;
mod tenant;

pub use github::{Change, GitHub};
pub use tenant::{Amount, GATE_PERCENT, Image, Replicas, Repo, Slo, Tenant, Tier};

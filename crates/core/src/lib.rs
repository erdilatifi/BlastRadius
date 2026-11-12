mod github;
mod tenant;

pub use github::{Change, GitHub};
pub use tenant::{Amount, Image, Replicas, Repo, Slo, Tenant, Tier};

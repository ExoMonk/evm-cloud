/// Map well-known chain names (and short aliases) to their numeric chain IDs.
pub(crate) fn chain_id(name: &str) -> Option<u64> {
    match name {
        "ethereum" | "eth" => Some(1),
        "polygon" | "matic" => Some(137),
        "arbitrum" | "arb" => Some(42161),
        "optimism" | "op" => Some(10),
        "base" => Some(8453),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_chains() {
        assert_eq!(chain_id("ethereum"), Some(1));
        assert_eq!(chain_id("polygon"), Some(137));
        assert_eq!(chain_id("arbitrum"), Some(42161));
        assert_eq!(chain_id("optimism"), Some(10));
        assert_eq!(chain_id("base"), Some(8453));
    }

    #[test]
    fn chain_aliases() {
        assert_eq!(chain_id("eth"), Some(1));
        assert_eq!(chain_id("matic"), Some(137));
        assert_eq!(chain_id("arb"), Some(42161));
        assert_eq!(chain_id("op"), Some(10));
    }

    #[test]
    fn unknown_chain_returns_none() {
        assert_eq!(chain_id("fantom"), None);
        assert_eq!(chain_id("solana"), None);
        assert_eq!(chain_id("avalanche"), None);
        assert_eq!(chain_id(""), None);
    }

    #[test]
    fn case_sensitive() {
        // chain_id is case-sensitive — uppercase should not match
        assert_eq!(chain_id("Ethereum"), None);
        assert_eq!(chain_id("POLYGON"), None);
    }
}

use sha3::{Digest, Keccak256};

pub fn selector4(signature: &str) -> String {
    let mut hasher = Keccak256::new();
    hasher.update(signature.as_bytes());
    let hash = hasher.finalize();
    hex::encode(&hash[..4])
}

pub fn builtin_selector(selector: &str) -> Option<&'static str> {
    match selector.to_ascii_lowercase().as_str() {
        "a9059cbb" => Some("transfer(address,uint256)"),
        "23b872dd" => Some("transferFrom(address,address,uint256)"),
        "095ea7b3" => Some("approve(address,uint256)"),
        "70a08231" => Some("balanceOf(address)"),
        "dd62ed3e" => Some("allowance(address,address)"),
        "18160ddd" => Some("totalSupply()"),
        "06fdde03" => Some("name()"),
        "95d89b41" => Some("symbol()"),
        "313ce567" => Some("decimals()"),
        "40c10f19" => Some("mint(address,uint256)"),
        "42842e0e" => Some("safeTransferFrom(address,address,uint256)"),
        "b88d4fde" => Some("safeTransferFrom(address,address,uint256,bytes)"),
        "3659cfe6" => Some("upgradeTo(address)"),
        "4f1ef286" => Some("upgradeToAndCall(address,bytes)"),
        "f851a440" => Some("admin()"),
        "5c60da1b" => Some("implementation()"),
        "59688b91" => Some("proxiableUUID()"),
        "715018a6" => Some("renounceOwnership()"),
        "8da5cb5b" => Some("owner()"),
        "f2fde38b" => Some("transferOwnership(address)"),
        _ => None,
    }
}

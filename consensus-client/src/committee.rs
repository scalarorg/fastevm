use rand::{SeedableRng as _, rngs::StdRng};
use consensus_config::{Authority, AuthorityKeyPair, Committee, NetworkKeyPair, ProtocolKeyPair};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitteeConfig {
    pub epoch: u64,
    pub authorities: Vec<AuthorityConfig>,
    pub docker_network: NetworkConfig,
    pub quorum_threshold: usize,
    pub validity_threshold: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthorityConfig {
    pub index: usize,
    pub stake: u64,
    pub hostname: String,
    pub address: String,
    pub authority_key: String,
    pub protocol_key: String,
    pub network_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub base_ip: String,
    pub start_ip: u8,
    pub end_ip: u8,
    pub port: u16,
}

/// Generates a committee configuration and saves it to a YAML file
pub fn generate_committees(
    output_path: &Path,
    authorities: usize,
    epoch: u64,
    stake: u64,
    docker_ips: &[String],
    network_ports: &[u16],
    hostname_prefix: &str,
) -> Result<()> {
    // Ensure we don't exceed the available Docker IPs
    if authorities > docker_ips.len() {
        return Err(anyhow::anyhow!(
            "Number of authorities ({}) exceeds available Docker IPs ({})",
            authorities,
            docker_ips.len()
        ));
    }

    if authorities > network_ports.len() {
        return Err(anyhow::anyhow!(
            "Number of authorities ({}) exceeds available network ports ({})",
            authorities,
            network_ports.len()
        ));
    }

    let mut authorities_config = vec![];
    let mut rng = StdRng::from_seed([0; 32]);

    for i in 0..authorities {
        let authority_keypair = AuthorityKeyPair::generate(&mut rng);
        let protocol_keypair = ProtocolKeyPair::generate(&mut rng);
        let network_keypair = NetworkKeyPair::generate(&mut rng);

        let address = format!("/ip4/{}/udp/{}", docker_ips[i], network_ports[i]);

        authorities_config.push(AuthorityConfig {
            index: i,
            stake,
            hostname: format!("{}{}", hostname_prefix, i),
            address,
            authority_key: format!("{:?}", authority_keypair.public()),
            protocol_key: format!("{:?}", protocol_keypair.public()),
            network_key: format!("{:?}", network_keypair.public()),
        });
    }

    let committee_config = CommitteeConfig {
        epoch,
        authorities: authorities_config,
        docker_network: NetworkConfig {
            base_ip: "172.20.0".to_string(),
            start_ip: 10,
            end_ip: 10 + authorities as u8 - 1,
            port: 26657,
        },
        quorum_threshold: (authorities * 2) / 3 + 1, // 2/3 + 1 for Byzantine fault tolerance
        validity_threshold: authorities / 2 + 1,      // 1/2 + 1 for validity
    };

    let yaml_content = serde_yaml::to_string(&committee_config)?;
    fs::write(output_path, yaml_content)?;

    println!("Committee configuration generated at: {}", output_path.display());
    println!("Configuration:");
    println!("  Epoch: {}", committee_config.epoch);
    println!("  Authorities: {}", committee_config.authorities.len());
    println!("  Stake per authority: {}", committee_config.authorities[0].stake);
    println!("  Quorum threshold: {}", committee_config.quorum_threshold);
    println!("  Validity threshold: {}", committee_config.validity_threshold);

    Ok(())
}

/// Loads a committee configuration from a YAML file
pub fn load_committees(config_path: &Path) -> Result<(Committee, Vec<(NetworkKeyPair, ProtocolKeyPair)>)> {
    let config_content = fs::read_to_string(config_path)?;
    let committee_config: CommitteeConfig = serde_yaml::from_str(&config_content)?;

    // Convert AuthorityConfig to Authority and generate keypairs
    let mut authorities = vec![];
    let mut key_pairs = vec![];
    let mut rng = StdRng::from_seed([0; 32]);

    for authority_config in committee_config.authorities {
        // Generate new keypairs (in a real scenario, you might want to load existing ones)
        let authority_keypair = AuthorityKeyPair::generate(&mut rng);
        let protocol_keypair = ProtocolKeyPair::generate(&mut rng);
        let network_keypair = NetworkKeyPair::generate(&mut rng);

        // Parse the address string
        let address = authority_config.address.parse()?;

        authorities.push(Authority {
            stake: authority_config.stake.into(),
            address,
            hostname: authority_config.hostname,
            authority_key: authority_keypair.public(),
            protocol_key: protocol_keypair.public(),
            network_key: network_keypair.public(),
        });
        key_pairs.push((network_keypair, protocol_keypair));
    }

    let committee = Committee::new(committee_config.epoch, authorities);
    
    println!("Loaded committee configuration from: {}", config_path.display());
    println!("Committee size: {}, Epoch: {}", committee.size(), committee.epoch());

    Ok((committee, key_pairs))
}

pub fn extract_peer_addresses(committee:&Committee) -> Vec<String> {
    committee.authorities()
        .map(|(_, authority)| {
            let address_str = authority.address.to_string();
            // Parse address in format "/ip4/172.20.0.11/udp/26657"
            if let Some(ip_port) = parse_ip_port_from_address(&address_str) {
                ip_port
            } else {
                // Fallback to original address if parsing fails
                address_str
            }
        })
        .collect()
}

/// Parse IP and port from address string in format "/ip4/172.20.0.11/udp/26657"
fn parse_ip_port_from_address(address: &str) -> Option<String> {
    // Split by "/" and extract IP and port
    let parts: Vec<&str> = address.split('/').collect();
    
    if parts.len() >= 5 && parts[1] == "ip4" && parts[3] == "udp" {
        let ip = parts[2];
        let port = parts[4];
        
        // Validate IP and port format
        if is_valid_ip(ip) && is_valid_port(port) {
            return Some(format!("{}:{}", ip, port));
        }
    }
    
    None
}

/// Check if string is a valid IP address
fn is_valid_ip(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    
    for part in parts {
        if part.parse::<u8>().is_err() {
            return false;
        }
    }
    true
}

/// Check if string is a valid port number
fn is_valid_port(port: &str) -> bool {
    port.parse::<u16>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use consensus_config::{Authority, AuthorityKeyPair, Committee, NetworkKeyPair, ProtocolKeyPair, Stake};

    // Helper function to create a test committee
    fn create_test_committee() -> Committee {
        let mut rng = StdRng::from_seed([0; 32]);
        let mut authorities = vec![];
        
        for i in 0..3 {
            let authority_keypair = AuthorityKeyPair::generate(&mut rng);
            let protocol_keypair = ProtocolKeyPair::generate(&mut rng);
            let network_keypair = NetworkKeyPair::generate(&mut rng);
            
            let address = format!("/ip4/172.20.0.{}/udp/{}", 10 + i, 26657 + i);
            let address = address.parse().unwrap();
            
            authorities.push(Authority {
                stake: Stake::from(1000u64),
                address,
                hostname: format!("test-node{}", i),
                authority_key: authority_keypair.public(),
                protocol_key: protocol_keypair.public(),
                network_key: network_keypair.public(),
            });
        }
        
        Committee::new(1, authorities)
    }

    #[test]
    fn test_extract_peer_addresses() {
        let committee = create_test_committee();
        let peer_addresses = extract_peer_addresses(&committee);
        
        assert_eq!(peer_addresses.len(), 3);
        assert_eq!(peer_addresses[0], "172.20.0.10:26657");
        assert_eq!(peer_addresses[1], "172.20.0.11:26658");
        assert_eq!(peer_addresses[2], "172.20.0.12:26659");
    }

    #[test]
    fn test_parse_ip_port_from_address_valid() {
        let test_cases = vec![
            ("/ip4/172.20.0.11/udp/26657", "172.20.0.11:26657"),
            ("/ip4/192.168.1.100/udp/8080", "192.168.1.100:8080"),
            ("/ip4/10.0.0.1/udp/9000", "10.0.0.1:9000"),
        ];
        
        for (input, expected) in test_cases {
            let result = parse_ip_port_from_address(input);
            assert_eq!(result, Some(expected.to_string()));
        }
    }

    #[test]
    fn test_parse_ip_port_from_address_invalid() {
        let test_cases = vec![
            "/ip4/172.20.0.11/tcp/26657",  // Wrong protocol
            "/ip6/172.20.0.11/udp/26657",  // Wrong IP version
            "/ip4/172.20.0.11/udp/",       // Missing port
            "/ip4/172.20.0.11/",           // Missing protocol and port
            "172.20.0.11:26657",           // Wrong format
            "",                             // Empty string
        ];
        
        for input in test_cases {
            let result = parse_ip_port_from_address(input);
            assert_eq!(result, None);
        }
    }

    #[test]
    fn test_parse_ip_port_from_address_malformed() {
        let test_cases = vec![
            "/ip4/172.20.0.11/udp/26657/extra",  // Extra parts - should still parse correctly
            "/ip4/172.20.0.11/udp",               // Missing port
            "/ip4/udp/26657",                     // Missing IP
        ];
        
        for input in test_cases {
            let result = parse_ip_port_from_address(input);
            if input == "/ip4/172.20.0.11/udp/26657/extra" {
                // This should still parse correctly as we only need the first 5 parts
                assert_eq!(result, Some("172.20.0.11:26657".to_string()));
            } else {
                assert_eq!(result, None);
            }
        }
    }

    #[test]
    fn test_is_valid_ip() {
        let valid_ips = vec![
            "127.0.0.1",
            "192.168.1.1",
            "10.0.0.1",
            "172.20.0.11",
            "0.0.0.0",
            "255.255.255.255",
        ];
        
        for ip in valid_ips {
            assert!(is_valid_ip(ip), "IP {} should be valid", ip);
        }
        
        let invalid_ips = vec![
            "256.1.2.3",      // Octet > 255
            "1.2.3.256",      // Octet > 255
            "1.2.3",          // Too few octets
            "1.2.3.4.5",      // Too many octets
            "1.2.3.a",        // Non-numeric
            "1.2.3.",         // Trailing dot
            ".1.2.3",         // Leading dot
            "",                // Empty string
        ];
        
        for ip in invalid_ips {
            assert!(!is_valid_ip(ip), "IP {} should be invalid", ip);
        }
    }

    #[test]
    fn test_is_valid_port() {
        let valid_ports = vec![
            "0",      // Min port
            "80",     // HTTP
            "443",    // HTTPS
            "26657",  // Tendermint
            "65535",  // Max port
        ];
        
        for port in valid_ports {
            assert!(is_valid_port(port), "Port {} should be valid", port);
        }
        
        let invalid_ports = vec![
            "65536",  // Too large
            "99999",  // Way too large
            "abc",    // Non-numeric
            "",       // Empty string
            "-1",     // Negative
        ];
        
        for port in invalid_ports {
            assert!(!is_valid_port(port), "Port {} should be invalid", port);
        }
    }

    #[test]
    fn test_extract_peer_addresses_with_invalid_addresses() {
        let mut rng = StdRng::from_seed([0; 32]);
        let mut authorities = vec![];
        
        // Create authorities with mixed valid and invalid addresses
        let addresses = vec![
            "/ip4/172.20.0.11/udp/26657",  // Valid
            "/ip4/127.0.0.1/udp/8080",     // Valid fallback
            "/ip4/192.168.1.100/udp/8080", // Valid
        ];
        
        for (i, address) in addresses.into_iter().enumerate() {
            let authority_keypair = AuthorityKeyPair::generate(&mut rng);
            let protocol_keypair = ProtocolKeyPair::generate(&mut rng);
            let network_keypair = NetworkKeyPair::generate(&mut rng);
            
            // Parse the address string
            let address = address.parse().unwrap();
            
            authorities.push(Authority {
                stake: Stake::from(1000u64),
                address,
                hostname: format!("test-node{}", i),
                authority_key: authority_keypair.public(),
                protocol_key: protocol_keypair.public(),
                network_key: network_keypair.public(),
            });
        }
        
        let committee = Committee::new(1, authorities);
        let peer_addresses = extract_peer_addresses(&committee);
        
        // Should have 3 addresses, all valid
        assert_eq!(peer_addresses.len(), 3);
        assert_eq!(peer_addresses[0], "172.20.0.11:26657");
        assert_eq!(peer_addresses[1], "127.0.0.1:8080");
        assert_eq!(peer_addresses[2], "192.168.1.100:8080");
    }

    #[test]
    fn test_edge_cases() {
        // Test with minimum valid values
        assert!(is_valid_ip("0.0.0.0"));
        assert!(is_valid_port("0"));
        
        // Test with maximum valid values
        assert!(is_valid_ip("255.255.255.255"));
        assert!(is_valid_port("65535"));
        
        // Test boundary cases
        assert!(!is_valid_ip("256.0.0.0"));
        assert!(!is_valid_port("65536"));
    }

    #[test]
    fn test_committee_config_serialization() {
        let config = CommitteeConfig {
            epoch: 1,
            authorities: vec![
                AuthorityConfig {
                    index: 0,
                    stake: 1000,
                    hostname: "test-node".to_string(),
                    address: "/ip4/172.20.0.11/udp/26657".to_string(),
                    authority_key: "key1".to_string(),
                    protocol_key: "key2".to_string(),
                    network_key: "key3".to_string(),
                }
            ],
            docker_network: NetworkConfig {
                base_ip: "172.20.0".to_string(),
                start_ip: 10,
                end_ip: 11,
                port: 26657,
            },
            quorum_threshold: 1,
            validity_threshold: 1,
        };
        
        // Test that the config can be serialized and deserialized
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: CommitteeConfig = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(config.epoch, deserialized.epoch);
        assert_eq!(config.authorities.len(), deserialized.authorities.len());
        assert_eq!(config.authorities[0].address, deserialized.authorities[0].address);
    }
}

//! Ordered IPv4 site policy compilation, classification and administrative inspection.
use crate::config::SiteRule;
use anyhow::{Result, ensure};
use std::net::IpAddr;

/// Independent connection admission and monitoring decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Classification {
    /// A matching forbid rule prevents all authentication.
    pub forbidden: bool,
    /// A matching suspect rule enables connection notices to the Suspect channel.
    pub suspect: bool,
}

/// One ordered rule retains its configuration identity for diagnostics.
#[derive(Clone, Debug)]
pub struct Rule {
    /// Literal configured address, never normalized to a network base.
    pub address: IpAddr,
    /// Bit mask; contiguous prefix bits are not required.
    pub mask: IpAddr,
    /// True denotes forbidden or suspected in the containing list.
    pub marked: bool,
    /// Source file and effective indexed configuration path.
    pub origin: String,
}
impl Rule {
    /// Match literal C address/mask semantics without network normalization.
    fn matches(&self, peer: IpAddr) -> bool {
        match (peer, self.address, self.mask) {
            (IpAddr::V4(peer), IpAddr::V4(address), IpAddr::V4(mask)) => {
                u32::from(peer) & u32::from(mask) == u32::from(address)
            }
            _ => false,
        }
    }
}

/// Immutable ordered access and suspicion lists, retained across Lua reloads.
#[derive(Clone, Debug, Default)]
pub struct Policy {
    /// First-match admission decisions.
    pub access: Vec<Rule>,
    /// First-match connection-monitoring decisions.
    pub suspicion: Vec<Rule>,
}
impl Policy {
    /// Compile the merged TOML declaration order, preserving array entry provenance.
    pub fn compile(
        table: Option<&toml::Value>,
        origins: &std::collections::BTreeMap<String, std::path::PathBuf>,
        warnings: &mut Vec<String>,
    ) -> Result<Self> {
        let mut policy = Self::default();
        if let Some(table) = table.and_then(toml::Value::as_table) {
            for (name, entries) in table {
                let (list, marked) = match name.as_str() {
                    "forbid" => (&mut policy.access, true),
                    "permit" => (&mut policy.access, false),
                    "suspect" => (&mut policy.suspicion, true),
                    "trust" => (&mut policy.suspicion, false),
                    _ => continue,
                };
                for (index, entry) in entries.as_array().unwrap().iter().enumerate() {
                    let parsed: SiteRule = entry.clone().try_into()?;
                    let path = format!("sites.{name}[{index}]");
                    let origin = format!(
                        "{}: {path}",
                        origins
                            .get(&path)
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    );
                    if let (IpAddr::V4(address), IpAddr::V4(mask)) = (parsed.address, parsed.mask)
                        && u32::from(address) & !u32::from(mask) != 0
                    {
                        warnings.push(format!(
                            "{origin}: address bits outside the mask make this rule unmatchable"
                        ));
                    }
                    list.push(Rule {
                        address: parsed.address,
                        mask: parsed.mask,
                        marked,
                        origin,
                    });
                }
            }
        }
        Ok(policy)
    }

    /// Reject unsupported enforcement before the server performs startup side effects.
    pub fn validate_listener(&self, address: IpAddr) -> Result<()> {
        for rule in self.access.iter().chain(&self.suspicion) {
            ensure!(
                rule.address.is_ipv4() && rule.mask.is_ipv4(),
                "{}: IPv6 site enforcement is unsupported; use IPv4 site rules",
                rule.origin
            );
        }
        ensure!(
            self.access.is_empty() && self.suspicion.is_empty() || address.is_ipv4(),
            "IPv6 listeners cannot enforce configured site rules; use an IPv4 listen address or remove site rules"
        );
        Ok(())
    }

    /// Classify once at acceptance; a miss is unrestricted and trusted.
    pub fn classify(&self, peer: IpAddr) -> Classification {
        Classification {
            forbidden: self
                .access
                .iter()
                .find(|r| r.matches(peer))
                .is_some_and(|r| r.marked),
            suspect: self
                .suspicion
                .iter()
                .find(|r| r.matches(peer))
                .is_some_and(|r| r.marked),
        }
    }

    /// Produce a private, bounded listing in actual first-match order.
    pub fn report(&self, limit: usize) -> Result<crate::reports::Report> {
        let mut report = crate::reports::Report::new(limit, "")?;
        for (title, rules, suspicion) in [
            ("Site Access", &self.access, false),
            ("Suspected Sites", &self.suspicion, true),
        ] {
            report.row(&format!("----- {title} -----"));
            report.row("Address              Mask                 Status");
            for rule in rules {
                let status = match (suspicion, rule.marked) {
                    (false, true) => "Forbidden",
                    (false, false) => "Unrestricted",
                    (true, true) => "Suspected",
                    (true, false) => "Trusted",
                };
                report.row(&format!(
                    "{:<20} {:<20} {status}",
                    rule.address.to_string(),
                    rule.mask.to_string()
                ));
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Rules retain literal C mask semantics, including noncontiguous masks.
    #[test]
    fn ordered_independent_classification_and_warnings() {
        let doc: toml::Value = toml::from_str("[sites]\ntrust=[{address='127.0.0.1',mask='255.255.255.255'}]\nsuspect=[{address='127.0.0.0',mask='255.0.0.0'}]\nforbid=[{address='10.0.2.0',mask='255.0.255.0'},{address='192.0.2.1',mask='255.255.255.0'}]\npermit=[{address='10.1.2.3',mask='255.255.255.255'}]").unwrap();
        let mut warnings = Vec::new();
        let policy = Policy::compile(doc.get("sites"), &Default::default(), &mut warnings).unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            policy.classify("127.0.0.1".parse().unwrap()),
            Classification::default()
        );
        assert!(policy.classify("127.0.0.2".parse().unwrap()).suspect);
        assert!(policy.classify("10.1.2.3".parse().unwrap()).forbidden);
        assert!(!policy.classify("192.0.2.1".parse().unwrap()).forbidden);
        assert_eq!(
            policy.classify("203.0.113.1".parse().unwrap()),
            Classification::default()
        );
        let report = policy.report(4096).unwrap().finish().unwrap();
        assert!(report.contains("----- Site Access -----") && report.contains("Trusted"));
        assert!(report.find("10.0.2.0").unwrap() < report.find("10.1.2.3").unwrap());
        assert!(
            policy
                .report(90)
                .unwrap()
                .finish()
                .unwrap()
                .contains("truncated")
        );
    }
}

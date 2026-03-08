//! Agent Skills support: discovery, parsing, and catalog generation.

mod discovery;
mod types;

pub use discovery::{discover_skills, extract_frontmatter, parse_skill_md};
pub use types::{SkillError, SkillRecord};

/// Build an XML skill catalog for injection into the system prompt.
///
/// Returns an empty string if there are no skills.
pub fn build_skill_catalog(skills: &[SkillRecord]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let mut catalog = String::from(
        "The following skills provide specialized instructions for specific tasks.\n\
         When a task matches a skill's description, call the activate_skill tool \
         with the skill's name to load its full instructions.\n\n\
         <available-skills>\n",
    );

    for skill in skills {
        // Use forward slashes for display consistency across platforms.
        let location = skill.location.to_string_lossy().replace('\\', "/");
        catalog.push_str(&format!(
            "  <skill name=\"{}\" location=\"{}\">\n    {}\n  </skill>\n",
            skill.name, location, skill.description,
        ));
    }

    catalog.push_str("</available-skills>");
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_skill_catalog_empty() {
        assert_eq!(build_skill_catalog(&[]), "");
    }

    #[test]
    fn test_build_skill_catalog_single() {
        let skills = vec![SkillRecord {
            name: "code-review".into(),
            description: "Review code for quality.".into(),
            location: PathBuf::from("/home/user/.krew/skills/code-review/SKILL.md"),
            base_dir: PathBuf::from("/home/user/.krew/skills/code-review"),
            compatibility: None,
            metadata: None,
        }];

        let catalog = build_skill_catalog(&skills);
        assert!(catalog.contains("<available-skills>"));
        assert!(catalog.contains("name=\"code-review\""));
        assert!(catalog.contains("Review code for quality."));
        assert!(catalog.contains("</available-skills>"));
        assert!(catalog.contains("activate_skill"));
    }

    #[test]
    fn test_build_skill_catalog_multiple() {
        let skills = vec![
            SkillRecord {
                name: "review".into(),
                description: "Review code.".into(),
                location: PathBuf::from("/skills/review/SKILL.md"),
                base_dir: PathBuf::from("/skills/review"),
                compatibility: None,
                metadata: None,
            },
            SkillRecord {
                name: "search".into(),
                description: "Search web.".into(),
                location: PathBuf::from("/skills/search/SKILL.md"),
                base_dir: PathBuf::from("/skills/search"),
                compatibility: None,
                metadata: None,
            },
        ];

        let catalog = build_skill_catalog(&skills);
        assert!(catalog.contains("name=\"review\""));
        assert!(catalog.contains("name=\"search\""));
    }
}

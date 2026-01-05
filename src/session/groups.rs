use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Group data (persisted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupData {
    pub name: String,
    pub path: String,
    pub expanded: bool,
    pub order: i32,
}

impl GroupData {
    pub fn new(path: String) -> Self {
        let name = path.split('/').last().unwrap_or(&path).to_string();
        Self {
            name,
            path,
            expanded: true,
            order: 0,
        }
    }
}

/// Group tree structure
#[derive(Debug, Clone)]
pub struct GroupTree {
    groups: HashMap<String, GroupData>,
}

impl GroupTree {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    /// Create from saved group data
    pub fn from_groups(groups: Vec<GroupData>) -> Self {
        let mut tree = Self::new();
        for group in groups {
            tree.groups.insert(group.path.clone(), group);
        }
        tree
    }

    /// Create a group
    pub fn create_group(&mut self, path: String) -> GroupData {
        if let Some(existing) = self.groups.get(&path) {
            return existing.clone();
        }

        let group = GroupData::new(path.clone());
        self.groups.insert(path, group.clone());

        // Ensure parent groups exist
        if let Some(parent_path) = self.parent_path(&group.path) {
            self.create_group(parent_path);
        }

        group
    }

    /// Delete a group
    pub fn delete_group(&mut self, path: &str) -> bool {
        self.groups.remove(path).is_some()
    }

    /// Get a group
    pub fn get_group(&self, path: &str) -> Option<&GroupData> {
        self.groups.get(path)
    }

    /// Get all groups
    pub fn all_groups(&self) -> Vec<GroupData> {
        let mut groups: Vec<_> = self.groups.values().cloned().collect();
        groups.sort_by(|a, b| a.order.cmp(&b.order).then(a.path.cmp(&b.path)));
        groups
    }

    /// Toggle group expansion
    pub fn toggle_expanded(&mut self, path: &str) {
        if let Some(group) = self.groups.get_mut(path) {
            group.expanded = !group.expanded;
        }
    }

    /// Set group expanded state
    pub fn set_expanded(&mut self, path: &str, expanded: bool) {
        if let Some(group) = self.groups.get_mut(path) {
            group.expanded = expanded;
        }
    }

    /// Check if group is expanded
    pub fn is_expanded(&self, path: &str) -> bool {
        self.groups.get(path).map(|g| g.expanded).unwrap_or(true)
    }

    /// Get parent path
    fn parent_path(&self, path: &str) -> Option<String> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() > 1 {
            Some(parts[..parts.len() - 1].join("/"))
        } else {
            None
        }
    }

    /// Get children of a group
    pub fn children(&self, path: &str) -> Vec<String> {
        let prefix = format!("{}/", path);
        self.groups
            .keys()
            .filter(|p| {
                p.starts_with(&prefix) && p.matches('/').count() == path.matches('/').count() + 1
            })
            .cloned()
            .collect()
    }

    /// Check if group has children
    pub fn has_children(&self, path: &str) -> bool {
        !self.children(path).is_empty()
    }

    /// Rename a group path prefix (renames the group and any descendants).
    pub fn rename_prefix(&mut self, old_prefix: &str, new_prefix: &str) -> bool {
        let old_prefix = old_prefix.trim();
        let new_prefix = new_prefix.trim();
        if old_prefix.is_empty() || old_prefix == new_prefix {
            return false;
        }

        let old_slash = format!("{}/", old_prefix);

        let mut updates: Vec<(String, GroupData)> = Vec::new();
        let mut removes: Vec<String> = Vec::new();

        for (path, data) in self.groups.iter() {
            if path == old_prefix || path.starts_with(&old_slash) {
                let suffix = &path[old_prefix.len()..];
                let new_path = format!("{new_prefix}{suffix}");

                let mut next = data.clone();
                next.path = new_path.clone();
                next.name = new_path.split('/').last().unwrap_or(&new_path).to_string();

                updates.push((new_path, next));
                removes.push(path.clone());
            }
        }

        for p in removes {
            self.groups.remove(&p);
        }

        for (p, g) in updates {
            self.groups.insert(p.clone(), g);
            if let Some(parent) = self.parent_path(&p) {
                self.create_group(parent);
            }
        }

        true
    }
}

impl Default for GroupTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_group() {
        let mut tree = GroupTree::new();
        tree.create_group("work".to_string());
        tree.create_group("work/frontend".to_string());

        assert!(tree.get_group("work").is_some());
        assert!(tree.get_group("work/frontend").is_some());
    }

    #[test]
    fn test_children() {
        let mut tree = GroupTree::new();
        tree.create_group("work".to_string());
        tree.create_group("work/frontend".to_string());
        tree.create_group("work/backend".to_string());

        let children = tree.children("work");
        assert_eq!(children.len(), 2);
        assert!(children.contains(&"work/frontend".to_string()));
        assert!(children.contains(&"work/backend".to_string()));
    }

    #[test]
    fn test_toggle_expanded() {
        let mut tree = GroupTree::new();
        tree.create_group("work".to_string());

        assert!(tree.is_expanded("work"));
        tree.toggle_expanded("work");
        assert!(!tree.is_expanded("work"));
    }

    #[test]
    fn test_rename_prefix() {
        let mut tree = GroupTree::new();
        tree.create_group("work".to_string());
        tree.create_group("work/frontend".to_string());
        tree.set_expanded("work/frontend", false);

        assert!(tree.rename_prefix("work", "jobs"));
        assert!(tree.get_group("jobs").is_some());
        assert!(tree.get_group("jobs/frontend").is_some());
        assert!(!tree.is_expanded("jobs/frontend"));
        assert!(tree.get_group("work").is_none());
    }
}

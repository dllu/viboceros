//! Use egui's accessibility tree to find and activate modeling dialog controls.
use super::*;
use egui::accesskit::{self, Node, NodeId, TreeUpdate};

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Target {
    Id(u64),
    Label(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Action {
    Click,
    Focus,
}

impl From<Action> for accesskit::Action {
    fn from(value: Action) -> Self {
        match value {
            Action::Click => Self::Click,
            Action::Focus => Self::Focus,
        }
    }
}

fn actionable(node: &Node) -> bool {
    [accesskit::Action::Click, accesskit::Action::Focus]
        .into_iter()
        .any(|action| node.supports_action(action))
}

impl Session {
    fn widget_tree(&mut self) -> Option<TreeUpdate> {
        self.frame(vec![], egui::Modifiers::NONE).accesskit_update
    }

    pub(super) fn widgets(&mut self) -> Value {
        let mut nodes = self
            .widget_tree()
            .into_iter()
            .flat_map(|update| update.nodes)
            .filter(|(_, node)| actionable(node))
            .collect::<Vec<(NodeId, Node)>>();
        nodes.sort_unstable_by(|(a_id, a), (b_id, b)| match (a.bounds(), b.bounds()) {
            (Some(a), Some(b)) => {
                a.y0.total_cmp(&b.y0)
                    .then(a.x0.total_cmp(&b.x0))
                    .then(a_id.cmp(b_id))
            }
            _ => a
                .bounds()
                .is_none()
                .cmp(&b.bounds().is_none())
                .then(a_id.cmp(b_id)),
        });
        json!(
            nodes
                .into_iter()
                .map(|(id, node)| json!({
                    "id": id.0,
                    "label": node.label(),
                    "role": format!("{:?}", node.role()),
                    "value": node.value(),
                    "enabled": !node.is_disabled(),
                    "clickable": node.supports_action(accesskit::Action::Click),
                    "focusable": node.supports_action(accesskit::Action::Focus),
                    "rectangle": node.bounds().map(|r| [r.x0, r.y0, r.x1, r.y1]),
                }))
                .collect::<Vec<_>>()
        )
    }

    pub(super) fn act_on_widget(&mut self, target: Target, action: Action) -> Result<()> {
        let action = action.into();
        let update = self.widget_tree().ok_or("no application widget tree")?;
        let mut matches = update
            .nodes
            .into_iter()
            .filter(|(_, node)| !node.is_disabled() && node.supports_action(action))
            .filter(|(id, node)| match &target {
                Target::Id(value) => id.0 == *value,
                Target::Label(label) => node.label() == Some(label.as_str()),
            });
        let (target_node, _) = matches
            .next()
            .ok_or("no enabled widget supports that action")?;
        if matches.next().is_some() {
            return Err("several widgets match this label; use an ID from widgets()".into());
        }
        self.frame(
            vec![egui::Event::AccessKitActionRequest(
                accesskit::ActionRequest {
                    action,
                    target_tree: accesskit::TreeId::ROOT,
                    target_node,
                    data: None,
                },
            )],
            egui::Modifiers::NONE,
        );
        Ok(())
    }
}

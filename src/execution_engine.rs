use std::collections::{HashMap, HashSet};

use egui_snarl::NodeId;

pub struct TaskDag {
    outstanding: HashMap<NodeId, HashSet<NodeId>>,
}

impl TaskDag {
    pub fn new(graph: &petgraph::Graph<NodeId, ()>) -> Self {
        let outstanding = graph
            .node_indices()
            .map(|idx| {
                let node_deps = graph
                    .neighbors_directed(idx, petgraph::Direction::Incoming)
                    .map(|idx| graph[idx])
                    .collect::<HashSet<_>>();
                (graph[idx], node_deps)
            })
            .collect();

        Self { outstanding }
    }

    /// List of tasks that have no outstanding dependencies
    pub fn ready_tasks(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.outstanding
            .iter()
            .filter(|(_task, pending_deps)| pending_deps.is_empty())
            .map(|(task, _)| *task)
    }

    /// Returns the list of tasks that are now ready to be started
    pub fn complete_task(&mut self, task: NodeId) -> HashSet<NodeId> {
        self.outstanding
            .remove(&task)
            .expect("completed task was still pending");

        // Remove the completed task from all dependents' lists
        let mut new_ready_tasks = HashSet::new();
        self.outstanding.iter_mut().for_each(|(id, dependencies)| {
            if dependencies.remove(&task) && dependencies.is_empty() {
                new_ready_tasks.insert(*id);
            }
        });

        new_ready_tasks
    }

    pub fn blocked_tasks(&mut self) -> impl Iterator<Item = NodeId> + '_ {
        self.outstanding
            .iter()
            .filter(|(_task, pending_deps)| !pending_deps.is_empty())
            .map(|(task, _)| *task)
    }
}

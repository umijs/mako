use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

#[derive(Clone)]
pub struct Bfs<N> {
    pub queue: VecDeque<N>,
    pub visited: Rc<RefCell<HashSet<N>>>,
}

pub enum NextResult<N> {
    Visited,
    First(N),
}

impl<N> Bfs<N>
where
    N: Eq + std::hash::Hash + Clone,
{
    pub fn new(queue: VecDeque<N>, visited: Rc<RefCell<HashSet<N>>>) -> Self {
        Bfs { queue, visited }
    }

    pub fn done(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn next_node(&mut self) -> NextResult<N> {
        let head = self.queue.pop_front().unwrap();
        if self.visited.borrow().contains(&head) {
            return NextResult::Visited;
        }
        self.visited.borrow_mut().insert(head.clone());
        NextResult::First(head)
    }

    pub fn visit(&mut self, node: N) {
        self.queue.push_back(node);
    }
}

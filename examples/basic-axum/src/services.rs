use thrust_macros::service;

/// The service depends on this abstraction, not a concrete repository.
/// Because the field below is written as a bare `dyn TodoRepository`, thrust
/// resolves it to the concrete impl at build time and compiles it as
/// `Arc<InMemoryTodoRepository>` — static dispatch, no trait object.
pub trait TodoRepository {
    fn all(&self) -> Vec<&'static str>;
}

#[service]
pub struct InMemoryTodoRepository;

impl TodoRepository for InMemoryTodoRepository {
    fn all(&self) -> Vec<&'static str> {
        vec!["buy milk", "write tests"]
    }
}

#[service]
pub struct TodoService {
    pub repo: dyn TodoRepository,
}

impl TodoService {
    pub fn find_all(&self) -> Vec<&'static str> {
        self.repo.all()
    }
}

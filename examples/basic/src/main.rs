mod services;

use services::*;

thrust_macros::init!();

fn main() {
    let container = Container::build();
    println!("{}", container.greeting_service.greet("world"));
}

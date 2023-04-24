use mako_bundler::plugin::{plugin_driver::PluginDriver, Plugin};
use std::{thread::sleep, time::Duration};

pub struct TestPlugin1;

impl Plugin for TestPlugin1 {
    fn name(&self) -> &str {
        "mako-test:plugin-1"
    }

    fn example_method(&self, prefix: String) -> Option<String> {
        Some(prefix + ":plugin-1")
    }
}

pub struct TestPlugin2;

impl Plugin for TestPlugin2 {
    fn name(&self) -> &str {
        "mako-test:plugin-2"
    }

    fn before(&self) -> &str {
        "mako-test:plugin-1"
    }

    fn example_method(&self, prefix: String) -> Option<String> {
        Some(prefix + ":plugin-2")
    }
}

#[test]
fn test_first_and_before() {
    let mut pd1 = PluginDriver::new();
    let mut pd2 = PluginDriver::new();

    // register in name order for pd1
    pd1.register(TestPlugin1 {});
    pd1.register(TestPlugin2 {});

    // register in before order for pd2
    pd2.register(TestPlugin1 {});
    pd2.register(TestPlugin2 {});

    // assert 2 plugin drivers get same order
    if let Some(ret) = pd1.run_hook_first(|p| p.example_method("p1".to_string())) {
        assert_eq!(ret, "p1:plugin-2")
    }
    if let Some(ret) = pd2.run_hook_first(|p| p.example_method("p2".to_string())) {
        assert_eq!(ret, "p2:plugin-2")
    }
}

#[test]
fn test_serial() {
    let mut pd = PluginDriver::new();

    pd.register(TestPlugin1 {});
    pd.register(TestPlugin2 {});

    let mut ret = vec![];
    pd.run_hook_serial(|p| {
        p.example_method("p".to_string()).map(|r| {
            ret.push(r);
            Some(())
        });
    });

    assert_eq!(ret, vec!["p:plugin-2", "p:plugin-1"]);
}

#[test]
fn test_parallel() {
    let mut pd = PluginDriver::new();

    pd.register(TestPlugin1 {});
    pd.register(TestPlugin2 {});

    let mut ret = vec![];
    pd.run_hook_serial(|p| {
        if p.name() == "mako-test:plugin-1" {
            // sleep 1ms to make sure plugin-1 run after plugin-2
            sleep(Duration::from_millis(1));
        }
        if let Some(r) = p.example_method("p".to_string()) {
            ret.push(r);
        }
    });

    assert_eq!(ret, vec!["p:plugin-2", "p:plugin-1"]);
}

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
    let ret1 = pd1.run_hook_first(|p| p.example_method("p1".to_string()));
    assert!(ret1.is_some());
    assert_eq!(ret1.unwrap(), "p1:plugin-2");

    let ret2 = pd2.run_hook_first(|p| p.example_method("p2".to_string()));
    assert!(ret2.is_some());
    assert_eq!(ret2.unwrap(), "p2:plugin-2");
}

#[test]
fn test_serial() {
    let mut pd = PluginDriver::new();

    pd.register(TestPlugin1 {});
    pd.register(TestPlugin2 {});

    let ret = pd.run_hook_serial(|p, last_ret| {
        if p.name() == "mako-test:plugin-2" {
            // sleep 1ms for plugin-2 to assert plugin-2 still run before plugin-1
            sleep(Duration::from_millis(1));
        }
        p.example_method(last_ret.unwrap_or_else(|| "serial".to_string()))
    });

    // expect hook return value is plugin method return value concat
    // and the plugin-1 is run after plugin-2
    assert!(ret.is_some());
    assert_eq!(ret.unwrap(), "serial:plugin-2:plugin-1");
}

#[test]
fn test_parallel() {
    static mut LAST_PLUGIN: String = String::new();
    let mut pd = PluginDriver::new();

    pd.register(TestPlugin1 {});
    pd.register(TestPlugin2 {});

    pd.run_hook_parallel(|p| {
        // sleep 1ms for plugin-2 to assert plugin-2 run after plugin-1
        if p.name() == "mako-test:plugin-2" {
            sleep(Duration::from_millis(1));
        }
        p.example_method("p".to_string());
        unsafe {
            LAST_PLUGIN = p.name().to_string();
        }
    });

    // expect plugin-2 is finished after plugin-1 even if it configured `before` plugin-1
    assert_eq!("mako-test:plugin-2", unsafe { LAST_PLUGIN.clone() });
}

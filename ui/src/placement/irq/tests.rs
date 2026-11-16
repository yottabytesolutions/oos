// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

const INTERRUPTS: &str = "           CPU0 CPU1 CPU2 CPU3 CPU4 CPU5
 78: 0 10 0 0 0 0 ITS-PCI-MSI 0 Edge xdma0
 31: 1 0 0 0 0 0 GICv3 1 Level eth0
 32: 1 0 0 0 0 0 GICv3 2 Level vop
 33: 1 0 0 0 0 0 GICv3 3 Level panfrost
 34: 1 0 0 0 0 0 GICv3 4 Level mmc
 76: 1 0 0 0 0 0 dummy xdma1
 Err: 0";

#[test]
fn only_xdma_msi_and_ethernet_change_in_fake_procfs() {
    let root = std::env::temp_dir().join(format!("oos-irq-{}", std::process::id()));
    let sys = root.join("sys");
    for cpu in 0..6 {
        let path = sys.join(format!("cpu{cpu}"));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("cpu_capacity"), if cpu < 4 { "485" } else { "1024" }).unwrap();
    }
    for irq in [78, 31, 32, 33, 34, 76] {
        let path = root.join(format!("irq/{irq}"));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("smp_affinity_list"), "0-5").unwrap();
    }
    let cores = Cores::of(&crate::placement::capacities(&sys));
    std::fs::write(root.join("interrupts"), INTERRUPTS).unwrap();
    let assignments = plan(cores, 0, &std::fs::read_to_string(root.join("interrupts")).unwrap());
    assert_eq!(assignments, [Assignment { irq: 78, cpu: 3 }, Assignment { irq: 31, cpu: 2 }]);
    assert!(apply(&root, &assignments).is_empty());
    for (irq, expected) in [(78, "3"), (31, "2"), (32, "0-5"), (33, "0-5"), (34, "0-5"), (76, "0-5")] {
        assert_eq!(std::fs::read_to_string(root.join(format!("irq/{irq}/smp_affinity_list"))).unwrap(), expected);
    }
    assert!(apply(&root, &[]).is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn irq_policy_avoids_the_supplied_busy_worker_and_handles_missing_cores() {
    let cores = Cores { big: 0x30, little: 15 };
    assert_eq!(plan(cores, 3, INTERRUPTS), [Assignment { irq: 78, cpu: 2 }, Assignment { irq: 31, cpu: 1 }]);
    assert!(plan(Cores::default(), 0, INTERRUPTS).is_empty());
    assert!(plan(Cores { little: 1, ..cores }, 0, INTERRUPTS).is_empty());
    let no_cpu0 = plan(Cores { little: 6, ..cores }, 1, INTERRUPTS);
    assert_eq!(no_cpu0, [Assignment { irq: 78, cpu: 2 }]);
    assert_eq!(apply(Path::new("/path/to/missing-procfs"), &no_cpu0).len(), 1);
}

#[test]
fn applied_irq_pairs_and_empty_plan_are_logged() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "placement::irq::tests::only_xdma_msi_and_ethernet_change_in_fake_procfs", "--nocapture"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(text.matches("irq_placement irq=78 cpu=3").count(), 1);
    assert_eq!(text.matches("irq_placement irq=31 cpu=2").count(), 1);
    assert_eq!(text.matches("irq_placement empty_plan").count(), 1);
}

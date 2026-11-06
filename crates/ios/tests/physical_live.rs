//! Live tests against approved physical devices. `PHONE_USE_TEST_IOS_EXPECT` lists `udid=tier` pairs, for example
//! `00008142-001971E2346B801C=coredevice,00008110-00142036217A201E=runner`.

use phone_use_ios::discovery;

fn expected() -> Vec<(String, String)> {
    std::env::var("PHONE_USE_TEST_IOS_EXPECT")
        .expect("PHONE_USE_TEST_IOS_EXPECT")
        .split(',')
        .map(|pair| {
            let (udid, tier) = pair.split_once('=').expect("udid=tier");
            (udid.to_string(), tier.to_string())
        })
        .collect()
}

#[tokio::test]
#[ignore = "needs approved physical iOS devices"]
async fn discovery_lists_each_wired_device_with_the_tier_its_os_and_services_allow() {
    let found = discovery::list().await.unwrap();
    for (udid, tier) in expected() {
        let device = found
            .iter()
            .find(|d| d.udid == udid)
            .unwrap_or_else(|| panic!("{udid} not listed in {found:?}"));
        assert_eq!(device.tier.name(), tier, "{device:?}");
        assert!(
            !device.name.is_empty() && !device.model.is_empty(),
            "{device:?}"
        );
        eprintln!("{device:?}");
    }
}

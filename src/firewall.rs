use log::{debug, error};
use windows::Win32::Foundation::FWP_E_ALREADY_EXISTS;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows::Win32::Foundation::VARIANT_BOOL;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWP_ACTION_PERMIT;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWP_CONDITION_VALUE0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWP_CONDITION_VALUE0_0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWP_MATCH_EQUAL;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWP_UINT16;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWPM_CONDITION_IP_LOCAL_PORT;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWPM_FILTER_CONDITION0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWPM_FILTER0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWPM_PROVIDER_FLAG_PERSISTENT;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWPM_PROVIDER0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWPM_FILTER_ENUM_TEMPLATE0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FwpmEngineClose0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FwpmEngineOpen0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FwpmFilterAdd0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FwpmFilterEnum0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FwpmFilterDeleteById0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FwpmFilterCreateEnumHandle0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FwpmProviderAdd0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FwpmProviderGetByKey0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FwpmTransactionBegin0;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::FwpmTransactionCommit0;
use windows::Win32::NetworkManagement::WindowsFirewall::INetFwRule;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_ACTION_ALLOW;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_IP_PROTOCOL_TCP;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_PROFILE2_ALL;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_PROFILE2_DOMAIN;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_PROFILE2_PRIVATE;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_PROFILE2_PUBLIC;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_RULE_DIR_IN;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_RULE_DIR_OUT;
use windows::Win32::NetworkManagement::WindowsFirewall::NetFwRule;
use windows::Win32::System::Com::*;
use windows::Win32::System::Rpc::RPC_C_AUTHN_DEFAULT;
use windows::core::BSTR;
use windows::core::GUID;
use windows::core::PWSTR;
use crate::error::Error;

/// Disables the Windows firewall by using the NetFwPolicy2 COM interface.
/// TODO: figure out if this even does anything meaningful.
pub(crate) fn disable_firewalls() -> Result<(), Error> {
    debug!("disabling firewall via NetFwPolicy2");

    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .map_err(|e| Error::ComInit(format!("CoInitializeEx failed: {e:?}")))?;

        let fw_policy: windows::Win32::NetworkManagement::WindowsFirewall::INetFwPolicy2 =
            windows::Win32::System::Com::CoCreateInstance(
                &windows::Win32::NetworkManagement::WindowsFirewall::NetFwPolicy2,
                None,
                windows::Win32::System::Com::CLSCTX_ALL,
            )
            .map_err(|e| Error::Firewall(format!("failed to create NetFwPolicy2: {e:?}")))?;

        for profile in [
            NET_FW_PROFILE2_DOMAIN,
            NET_FW_PROFILE2_PUBLIC,
            NET_FW_PROFILE2_PRIVATE,
        ] {
            debug!(
                "Profile {:?} is blocking inbound traffic? {:?}",
                profile,
                fw_policy.get_BlockAllInboundTraffic(profile)
            );
            debug!(
                "Profile {:?} is enabled {:?}",
                profile,
                fw_policy.get_FirewallEnabled(profile)
            );
            debug!(
                "Profile {:?} default inbound action {:?}",
                profile,
                fw_policy.get_DefaultInboundAction(profile)
            );
            debug!(
                "Profile {:?} notifications disabled {:?}",
                profile,
                fw_policy.get_NotificationsDisabled(profile)
            );

            fw_policy
                .put_BlockAllInboundTraffic::<VARIANT_BOOL>(profile, false.into())
                .map_err(|e| {
                    Error::Firewall(format!(
                        "failed to reset inbound traffic block for {profile:?}: {e:?}"
                    ))
                })?;

            fw_policy
                .put_FirewallEnabled::<VARIANT_BOOL>(profile, false.into())
                .map_err(|e| {
                    Error::Firewall(format!(
                        "failed to disable firewall for {profile:?}: {e:?}"
                    ))
                })?;

            fw_policy
                .put_DefaultInboundAction(profile, NET_FW_ACTION_ALLOW)
                .map_err(|e| {
                    Error::Firewall(format!(
                        "failed to set default inbound action for {profile:?}: {e:?}"
                    ))
                })?;

            fw_policy
                .put_NotificationsDisabled::<VARIANT_BOOL>(profile, true.into())
                .map_err(|e| {
                    Error::Firewall(format!("failed to disable notifications: {e:?}"))
                })?;

            let rules = fw_policy
                .Rules()
                .map_err(|e| Error::Firewall(format!("failed to get firewall rules: {e:?}")))?;

            if rules
                .Item(&BSTR::from("AllowAnyProgramAnyPortCOMIn"))
                .is_ok()
                || rules
                    .Item(&BSTR::from("AllowAnyProgramAnyPortCOMOut"))
                    .is_ok()
            {
                // Rules were already created
                debug!("AllowAnyProgramAnyPort rules for {profile:?} already exist, continuing");
                continue;
            }

            for (direction_name, direction) in
                [("In", NET_FW_RULE_DIR_IN), ("Out", NET_FW_RULE_DIR_OUT)]
            {
                debug!("Adding rule in {direction_name:?} direction");

                let rule: INetFwRule =
                    CoCreateInstance(&NetFwRule, None, CLSCTX_ALL).map_err(|e| {
                        Error::Firewall(format!("creating NetFwRule instance: {e:?}"))
                    })?;

                rule.SetEnabled::<VARIANT_BOOL>(true.into())
                    .map_err(|e| Error::Firewall(format!("SetEnabled failed: {e:?}")))?;

                rule.SetName(&BSTR::from(format!(
                    "AllowAnyProgramAnyPortCOM{direction_name}"
                )))
                .map_err(|e| Error::Firewall(format!("SetName failed: {e:?}")))?;
                rule.SetApplicationName(&BSTR::from("C:\\Windows\\system32\\conhost.exe"))
                    .map_err(|e| Error::Firewall(format!("SetApplicationName failed: {e:?}")))?;
                rule.SetProtocol(NET_FW_IP_PROTOCOL_TCP.0)
                    .map_err(|e| Error::Firewall(format!("SetProtocol failed: {e:?}")))?;
                rule.SetAction(NET_FW_ACTION_ALLOW)
                    .map_err(|e| Error::Firewall(format!("SetAction failed: {e:?}")))?;
                rule.SetDirection(direction)
                    .map_err(|e| Error::Firewall(format!("SetDirection failed: {e:?}")))?;
                rule.SetDescription(&BSTR::from("Testing"))
                    .map_err(|e| Error::Firewall(format!("SetDescription failed: {e:?}")))?;
                rule.SetGrouping(&BSTR::from("Xbox SystemOS"))
                    .map_err(|e| Error::Firewall(format!("SetGrouping failed: {e:?}")))?;
                rule.SetProfiles(NET_FW_PROFILE2_ALL.0)
                    .map_err(|e| Error::Firewall(format!("SetProfiles failed: {e:?}")))?;
                // rule.SetRemoteAddresses(&BSTR::from("LocalSubnet"))
                //     .context("SetRemoteAddresses")?;
                // rule.SetLocalAddresses(&BSTR::from("LocalSubnet"))
                //     .context("SetRemoteAddresses")?;
                // rule.SetInterfaceTypes(&BSTR::from("All"))
                //     .context("SetInterfaceTypes")?;

                rules
                    .Add(&rule)
                    .map_err(|e| Error::Firewall(format!("Add rule failed: {e:?}")))?;
            }
        }
    }

    debug!("successfully disabled firewall");

    Ok(())
}

fn provider_guid() -> GUID {
    GUID::from_values(
        0xabad1dea,
        0x4141,
        0x4141,
        [0x0, 0x0, 0x0c, 0x0f, 0xfe, 0xe0, 0x00, 0x00],
    )
}

fn open_fwp_session() -> HANDLE {
    let mut handle = INVALID_HANDLE_VALUE;

    let res = unsafe {
        FwpmEngineOpen0(
            None,
            RPC_C_AUTHN_DEFAULT as u32,
            None,
            None,
            &mut handle as *mut _,
        )
    };

    debug!("FwpmEngineOpen0 result: 0x{res:08X}");

    handle
}

fn build_and_add_fwp_port_filter(name: &str, ports: &[u16], layer: GUID, engine: HANDLE) {
    let mut provider_key = provider_guid();

    let mut filter: FWPM_FILTER0 = unsafe { core::mem::zeroed() };
    let mut name: Vec<u16> = name.encode_utf16().collect();
    name.push(0x0);

    filter.displayData.name = PWSTR::from_raw(name.as_mut_ptr());
    filter.providerKey = &mut provider_key as *mut _;
    filter.layerKey = layer;

    // Create a Vec of FWPM_FILTER_CONDITION0, one for each port
    let mut conditions: Vec<FWPM_FILTER_CONDITION0> = ports
        .iter()
        .map(|&port| FWPM_FILTER_CONDITION0 {
            fieldKey: FWPM_CONDITION_IP_LOCAL_PORT,
            matchType: FWP_MATCH_EQUAL,
            conditionValue: FWP_CONDITION_VALUE0 {
                r#type: FWP_UINT16,
                Anonymous: FWP_CONDITION_VALUE0_0 { uint16: port },
            },
        })
        .collect();

    filter.numFilterConditions = conditions.len() as u32;
    filter.filterCondition = conditions.as_mut_ptr();
    filter.action.r#type = FWP_ACTION_PERMIT;

    unsafe {
        let mut filter_id = 0u64;
        let res = FwpmFilterAdd0(engine, &filter, None, Some(&mut filter_id as *mut _));

        debug!("FwpmFilterAdd0 res: 0x{res:08X}");
        debug!("Filter ID: {filter_id}");
    }
}

fn install_fwpm_provider(engine: HANDLE) {
    unsafe {
        let mut provider_ptr: *mut FWPM_PROVIDER0 = std::ptr::null_mut();
        let ret = FwpmProviderGetByKey0(engine, &provider_guid() as *const _, &mut provider_ptr);
        if ret == 0 && !provider_ptr.is_null() {
            debug!("FPM provider already exists");
            return;
        }
    }

    let mut name: Vec<u16> = "LittleHydra".encode_utf16().collect();
    name.push(0x0);

    let mut provider: FWPM_PROVIDER0 = unsafe { core::mem::zeroed() };
    provider.providerKey = provider_guid();
    provider.displayData.name = PWSTR::from_raw(name.as_mut_ptr());
    provider.flags = FWPM_PROVIDER_FLAG_PERSISTENT;

    unsafe {
        let res = FwpmTransactionBegin0(engine, 0);
        debug!("FwpmTransactionBegin0 res: 0x{res:08X}");

        let res = FwpmProviderAdd0(engine, &provider as *const _, None);
        // We can safely ignore this
        if res != FWP_E_ALREADY_EXISTS.0 as u32 {
            debug!("FwpmProviderAdd0 res: 0x{res:08X}");
        } else {
            debug!("FwpmProviderAdd0 res: FWP_E_ALREADY_EXISTS");
        }

        let res = FwpmTransactionCommit0(engine);
        debug!("FwpmTransactionCommit0 res: 0x{res:08X}");
    }
}

pub(crate) fn allow_ports_through_firewall(name: &str, ports: &[u16]) -> Result<(), Error> {
    let engine = open_fwp_session();
    debug!("Engine HANDLE: {engine:#X?}");
    install_fwpm_provider(engine);

    build_and_add_fwp_port_filter(name, ports, FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4, engine);

    unsafe {
        FwpmEngineClose0(engine);
    }

    Ok(())
}

/// Removes a port filter from the firewall by its display name.
pub(crate) fn delete_rule_from_firewall(name: &str, engine: HANDLE) -> Result<(), Error> {
    unsafe {
        let mut enum_template: FWPM_FILTER_ENUM_TEMPLATE0 = core::mem::zeroed();
        enum_template.layerKey = FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4;
        let mut enum_handle = HANDLE::default();
        let res = FwpmFilterCreateEnumHandle0(
            engine,
            Some(&enum_template),
            &mut enum_handle,
        );
        if res != 0 {
            return Err(Error::Firewall(format!("FwpmFilterCreateEnumHandle0 failed: 0x{res:08X}")));
        }

        let mut filters_ptr: *mut *mut FWPM_FILTER0 = std::ptr::null_mut();
        let mut num_filters: u32 = 0;
        let num_entries_requested = 256;
        let res = FwpmFilterEnum0(
            engine,
            enum_handle,
            num_entries_requested,
            &mut filters_ptr,
            &mut num_filters,
        );
        if res != 0 {
            return Err(Error::Firewall(format!("FwpmFilterEnum0 (fetch) failed: 0x{res:08X}")));
        }

        let filters = std::slice::from_raw_parts(filters_ptr, num_filters as usize);
        let mut found = false;
        for &filter_ptr in filters {
            if filter_ptr.is_null() {
                continue;
            }
            let filter = &*filter_ptr;
            if !filter.displayData.name.is_null() {
                let filter_name = {
                    let mut len = 0;
                    let mut ptr = filter.displayData.name.0;
                    while !ptr.is_null() && *ptr != 0 {
                        len += 1;
                        ptr = ptr.add(1);
                    }
                    let slice = std::slice::from_raw_parts(filter.displayData.name.0, len);
                    String::from_utf16_lossy(slice)
                };
                if filter_name == name {
                    let res = FwpmFilterDeleteById0(engine, filter.filterId);
                    if res != 0 {
                        debug!("FwpmFilterDeleteById0 failed: 0x{res:08X}");
                        return Err(Error::Firewall(format!("FwpmFilterDeleteById0 failed: 0x{res:08X}")));
                    }
                    found = true;
                    debug!("Deleted filter with name: {name}");
                    break;
                }
            }
        }
        if !found {
            return Err(Error::Firewall(format!("No filter found with name: {name}")));
        }
    }
    Ok(())
}

pub(crate) fn remove_port_from_firewall_by_name(name: &str) -> Result<(), Error> {
    let engine = open_fwp_session();
    debug!("Engine HANDLE: {engine:#X?}");
    install_fwpm_provider(engine);

    if let Err(err) = delete_rule_from_firewall(name, engine) {
        error!("Failed to delete filter by name '{name}', err: {err:?}");
    }

    unsafe {
        FwpmEngineClose0(engine);
    }

    Ok(())
}

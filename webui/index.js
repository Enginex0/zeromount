import { exec, toast, listPackages, getPackagesInfo } from 'kernelsu-alt';
import '@material/web/button/filled-button.js';
import '@material/web/button/outlined-button.js';
import '@material/web/iconbutton/outlined-icon-button.js';
import '@material/web/icon/icon.js';
import '@material/web/iconbutton/icon-button.js';
import '@material/web/switch/switch.js';
import '@material/web/fab/fab.js';
 
const ADB_DIR = "/data/adb";
const MOD_DIR = `${ADB_DIR}/modules`;
const NM_DATA = `${ADB_DIR}/nomount`;
const NM_BIN = `${MOD_DIR}/nomount/bin/nm`;
const FILES = {
    verbose: `${NM_DATA}/.verbose`,
    disable: `${NM_DATA}/disable`,
    exclusions: `${NM_DATA}/.exclusion_list`,
    hot_install: `${NM_DATA}/.hot_install`
};

function showToast(msg) {
    try { toast(msg); } catch (e) { console.log(`[TOAST]: ${msg}`); }
}

function initNavigation() {
    const navItems = document.querySelectorAll('.nav-item');
    const views = document.querySelectorAll('.view-content');
    const fabContainer = document.getElementById('fab-container');

    navItems.forEach(item => {
        item.addEventListener('click', () => {
            navItems.forEach(nav => nav.classList.remove('active'));
            item.classList.add('active');

            const targetId = item.dataset.target;
            views.forEach(view => {
                if (view.id === targetId) {
                    view.classList.add('active');
                    if (targetId === 'view-exclusions') {
                        fabContainer.classList.add('visible');
                        loadExclusions();
                    } else {
                        fabContainer.classList.remove('visible');
                    }
                    if (targetId === 'view-modules') loadModules();
                    if (targetId === 'view-options') loadOptions();
                    if (targetId === 'view-home') loadHome();
                } else {
                    view.classList.remove('active');
                }
            });
        });
    });
}

async function loadHome() {
    const script = `

        uname -r
        echo "|||"

        ${NM_BIN} v
        echo "|||"

        grep "version=" ${MOD_DIR}/nomount/module.prop | head -n1 | cut -d= -f2
        echo "|||"

        val=$(getprop ro.product.vendor.model)
        [ -z "$val" ] && val=$(getprop ro.product.model)
        echo "$val"
        echo "|||"

        getprop ro.build.version.release
        echo "|||"

        getprop ro.build.version.sdk
        echo "|||"

        active_rules=$(${NM_BIN} list)
        
        count=0
        cd ${MOD_DIR}
        for mod in *; do
            if [ ! -d "$mod" ] || [ "$mod" = "nomount" ]; then continue; fi
            if echo "$active_rules" | grep -qF "${MOD_DIR}/$mod/"; then
                count=$((count + 1))
            fi
        done
        echo "$count"
    `;

    try {
        const result = await exec(script);

        const parts = result.stdout.split('|||').map(s => s.trim());

        if (parts.length < 7) throw new Error("Incomplete system data");

        const [
            kernelVer, 
            driverVer, 
            modVer, 
            deviceModel, 
            androidVer, 
            apiLvl, 
            activeModulesCount
        ] = parts;

        document.getElementById('kernel-version').textContent = kernelVer || "Unknown";
        
        const driverText = driverVer || "Unknown";
        const modText = modVer || "v0.0.0";
        const indicator = document.getElementById('status-indicator');
        const versionDisplay = document.getElementById('nomount-version');

        versionDisplay.textContent = `${modText} (${driverText})`;

        if (driverText !== "Unknown") {
            indicator.textContent = "Active";
            indicator.style.color = "var(--md-sys-color-primary)";
        } else {
            indicator.textContent = "Inactive";
            indicator.style.color = "var(--md-sys-color-error)";
        }

        document.getElementById('device-model').textContent = deviceModel || "Unknown Device";
        document.getElementById('android-ver').textContent = `Android ${androidVer} (API ${apiLvl})`;

        document.getElementById('injection-stats').textContent = `${activeModulesCount} modules injecting`;

    } catch (e) {
        console.error("Error loading Home:", e);
    }
}

async function loadModules() {
    const listContainer = document.getElementById('modules-list');
    const emptyBanner = document.getElementById('modules-empty');
    
    const script = `
        active_rules=$(${NM_BIN} list)
        
        cd ${MOD_DIR}
        for mod in *; do
            if [ ! -d "$mod" ] || [ "$mod" = "nomount" ]; then continue; fi
            if [ -d "$mod/system" ] || [ -d "$mod/vendor" ] || \
               [ -d "$mod/product" ] || [ -d "$mod/system_ext" ] || \
               [ -d "$mod/oem" ] || [ -d "$mod/odm" ]; then
               
               name=$(grep "^name=" "$mod/module.prop" | head -n1 | cut -d= -f2-)
               if [ -f "$mod/disable" ]; then enabled="false"; else enabled="true"; fi

               file_list=$(find "$mod/system" "$mod/vendor" "$mod/product" "$mod/system_ext" -type f 2>/dev/null)
               potential_count=$(echo "$file_list" | wc -l)

               if echo "$active_rules" | grep -qF "${MOD_DIR}/$mod/"; then
                   is_loaded="true"
                   count=$potential_count
               else
                   is_loaded="false"
                   count=0
               fi
               
               echo "$mod|$name|$enabled|$count|$is_loaded"
            fi
        done
    `;

    try {
        const result = await exec(script);
        const lines = result.stdout.split('\n').filter(line => line.trim() !== '');
        
        listContainer.innerHTML = '';
        listContainer.appendChild(emptyBanner);

        if (lines.length === 0) {
            emptyBanner.classList.add('active');
            return;
        }
        emptyBanner.classList.remove('active');

        const fragment = document.createDocumentFragment();

        lines.forEach(line => {
            let [modId, realName, enabledStr, fileCount, loadedStr] = line.split('|');
            
            realName = (realName || modId).trim();
            const isEnabled = enabledStr.trim() === 'true'; 
            const isLoaded = loadedStr.trim() === 'true'; 
            const count = parseInt(fileCount) || 0;

            const card = document.createElement('div');
            card.className = 'card module-card';
            
            card.innerHTML = `
                <div class="module-header">
                    <div class="module-info">
                        <h3>${realName}</h3>
                        <p>${modId}</p>
                    </div>
                    <md-switch id="switch-${modId}" ${isEnabled ? 'selected' : ''}></md-switch>
                </div>

                <div class="module-divider"></div>

                <div class="module-extension">
                    <div class="file-count">
                        <md-icon style="font-size:16px;">description</md-icon>
                        <span>${count} file${count !== 1 ? 's' : ''} injected</span>
                    </div>
                    
                    <button class="btn-hot-action ${isLoaded ? 'unload' : ''}" id="btn-hot-${modId}">
                        ${isLoaded ? 'Hot Unload' : 'Hot Load'}
                    </button>
                </div>
            `;

            const toggle = card.querySelector(`#switch-${modId}`);
            toggle.addEventListener('change', async () => {
                toggle.disabled = true;
                try {
                    if (toggle.selected) {
                        await exec(`rm ${MOD_DIR}/${modId}/disable`);
                        await loadModule(modId);
                        showToast(`${realName} Enabled & Loaded`);
                    } else {
                        await unloadModule(modId);
                        await exec(`touch ${MOD_DIR}/${modId}/disable`);
                        showToast(`${realName} Unloaded & Disabled`);
                    }
                } catch (e) {
                    showToast(`Error: ${e.message}`);
                } finally {
                    loadModules();
                }
            });

            const hotBtn = card.querySelector(`#btn-hot-${modId}`);
            hotBtn.addEventListener('click', async () => {
                hotBtn.textContent = "...";
                hotBtn.disabled = true;
                try {
                    if (isLoaded) {
                        await unloadModule(modId);
                        showToast(`${realName} Unloaded`);
                    } else {
                        await loadModule(modId);
                        showToast(`${realName} Loaded`);
                    }
                } catch (e) {
                    showToast(`Action failed: ${e.message}`);
                } finally {
                    loadModules();
                }
            });

            fragment.appendChild(card);
        });

        listContainer.appendChild(fragment);

    } catch (e) {
        console.error("Error loading modules:", e);
        listContainer.innerHTML = `<div style="padding:20px; color:var(--md-sys-color-error);">Error loading modules: ${e.message}</div>`;
    }                                     
}

async function loadModule(modId) {
    const script = `
        cd ${MOD_DIR}/${modId}
        for part in system vendor product system_ext oem odm; do
            if [ -d "$part" ]; then
                find "$part" -type f | while read -r file; do
                    target="/$file"
                    source="${MOD_DIR}/${modId}/$file"
                    ${NM_BIN} add "$target" "$source"
                done
            fi
        done
    `;
    await exec(script);
}

async function unloadModule(modId) {
    const script = `
        active_rules=$(${NM_BIN} list)
        echo "$active_rules" | while read -r rule; do
            if [ -z "$rule" ]; then continue; fi
            real_path=\${rule%%->*}
            virtual_path=\${rule#*->}
            if echo "$real_path" | grep -qF "${MOD_DIR}/${modId}/"; then
                ${NM_BIN} del "$virtual_path"
            fi
        done
    `;
    await exec(script);
}

let allAppsCache = [];
let showSystemApps = false;

async function loadExclusions() {
    const listContainer = document.getElementById('exclusions-list');
    const cat = await exec(`cat ${FILES.exclusions}`);
    const blockedUids = cat.stdout.split('\n').filter(u => u.trim() !== '');

    if (blockedUids.length === 0) {
        listContainer.innerHTML = '<div style="opacity:0.5; text-align:center; padding:20px;">No exclusions yet</div>';
        return;
    }

    if (allAppsCache.length === 0) {
        listContainer.innerHTML = '<div style="padding:20px; text-align:center; opacity:0.6;">Loading apps...</div>';
        
        try {
            const packages = await listPackages();
            allAppsCache = await getPackagesInfo(packages);
        } catch (e) {
            console.warn("Error getting app info:", e);
        }
    }

    listContainer.innerHTML = ''; 

    blockedUids.forEach(uid => {
        const appInfo = allAppsCache.find(a => a.uid == uid);
        const label = appInfo ? (appInfo.appLabel || appInfo.packageName) : `Unknown (UID: ${uid})`;
        const pkg = appInfo ? appInfo.packageName : 'App not installed or found';
        const iconSrc = appInfo ? `ksu://icon/${appInfo.packageName}` : '';
        const fallbackIcon = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0iIzgwODA4MCI+PHBhdGggZD0iTTEyIDJDNi40OCAyIDIgNi40OCAyIDEyczQuNDggMTA 5MTAgMTAgMTAtNC40OCAxMC0xMFMxNy41MiAyIDEyIDJ6bTAgMThjLTQuNDEgMC04LTMuNTktOC04czMuNTktOCA4LTggOCAzLjU5IDggOC0zLjU5IDgtOCA4eiIvPjwvc3ZnPg==";

        const item = document.createElement('div');
        item.className = 'card setting-item';
        item.innerHTML = `
            <div style="display:flex; align-items:center; gap:16px; overflow: hidden;">
                <img src="${iconSrc}" class="app-icon-img" style="width: 40px; height: 40px; border-radius: 10px;" loading="lazy" onerror="this.src='${fallbackIcon}'" />
                
                <div class="setting-text" style="overflow: hidden;">
                    <h3 style="white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">${label}</h3>
                    <p style="font-size: 12px; opacity: 0.7; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">${pkg}</p>
                    <p style="font-size: 10px; color: var(--md-sys-color-error); margin-top: 2px;">Blocked</p>
                </div>
            </div>

            <md-icon-button class="btn-delete">
                <md-icon>delete</md-icon>
            </md-icon-button>
        `;
        
        item.querySelector('.btn-delete').addEventListener('click', async () => {
            item.style.opacity = '0.5';
            item.style.pointerEvents = 'none';
            
            await exec(`sed -i "/${uid}/d" ${FILES.exclusions}`);
            await exec(`${NM_BIN} unblock ${uid}`);
            loadExclusions();
        });

        listContainer.appendChild(item);
    });
}

async function openAppSelector() {
    const modal = document.getElementById('app-selector-modal');
    const container = document.getElementById('app-list-container');
    const searchInput = document.getElementById('app-search-input');
    const filterMenu = document.getElementById('filter-menu');
    const filterBtn = document.getElementById('btn-filter-toggle');
    const sysSwitch = document.getElementById('switch-system-apps');

    modal.classList.add('active');

    filterMenu.classList.remove('active'); 
    searchInput.value = '';
    sysSwitch.selected = showSystemApps;

    container.innerHTML = '<div class="loading-spinner" style="padding:20px; text-align:center;">Loading apps...</div>';
    try {
        if (!allAppsCache || allAppsCache.length === 0) {
            const packages = await listPackages();
            allAppsCache = await getPackagesInfo(packages);
            allAppsCache.sort((a, b) => (a.appLabel || a.packageName).localeCompare(b.appLabel || b.packageName));
        }

        filterAndRender();

        searchInput.oninput = (e) => {
            filterAndRender(e.target.value);
        };

        filterBtn.onclick = () => {
            filterMenu.classList.toggle('active');
        };

        sysSwitch.onchange = () => {
            showSystemApps = sysSwitch.selected;
            filterAndRender(searchInput.value);
        };

    } catch (e) {
        container.innerHTML = `<div style="padding:20px; color:var(--md-sys-color-error);">Error: ${e.message}</div>`;
        console.error(e);
    }
}

function filterAndRender(searchTerm = '') {
    const term = searchTerm.toLowerCase();
    const filtered = allAppsCache.filter(app => {
        const matchesSearch = (app.appLabel || "").toLowerCase().includes(term) || 
                              (app.packageName || "").toLowerCase().includes(term);

        const matchesType = showSystemApps ? true : !app.isSystem;
        return matchesSearch && matchesType;
    });

    renderAppList(filtered);
}

function renderAppList(apps) {
    const container = document.getElementById('app-list-container');
    container.innerHTML = '';
    
    if (apps.length === 0) {
        container.innerHTML = '<div style="padding:20px; text-align:center; opacity:0.6;">No apps found</div>';
        return;
    }

    const limit = 100; 
    const appsToShow = apps.slice(0, limit);

    const fragment = document.createDocumentFragment();

    appsToShow.forEach(app => {
        const item = document.createElement('div');
        item.className = 'app-item';
        
        const iconSrc = `ksu://icon/${app.packageName}`;
        const fallback = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0iIzgwODA4MCI+PHBhdGggZD0iTTEyIDJDNi40OCAyIDIgNi40OCAyIDEyczQuNDggMTA 5MTAgMTAgMTAtNC40OCAxMC0xMFMxNy41MiAyIDEyIDJ6bTAgMThjLTQuNDEgMC04LTMuNTktOC04czMuNTktOCA4LTggOCAzLjU5IDggOC0zLjU5IDgtOCA4eiIvPjwvc3ZnPg==";

        item.innerHTML = `
            <img src="${iconSrc}" class="app-icon-img" loading="lazy" onerror="this.src='${fallback}'" /> 
            <div class="app-details">
                <span class="app-name">${app.appLabel || app.packageName}</span>
                <span class="app-pkg">${app.packageName}</span>
            </div>
            <div style="text-align:right;">
                <div style="font-size: 12px; color: var(--md-sys-color-primary);">UID: ${app.uid}</div>
                ${app.isSystem ? '<span style="font-size:10px; background:#333; padding:2px 4px; border-radius:4px; opacity:0.7;">SYS</span>' : ''}
            </div>
        `;

        item.addEventListener('click', async () => {
            await addExclusion(app.uid, app.appLabel || app.packageName);
            document.getElementById('app-selector-modal').classList.remove('active');
        });

        fragment.appendChild(item);
    });
    
    container.appendChild(fragment);
    if (apps.length > limit) {
        const moreInfo = document.createElement('div');
        moreInfo.style.textAlign = 'center';
        moreInfo.style.padding = '10px';
        moreInfo.style.opacity = '0.5';
        moreInfo.style.fontSize = '12px';
        moreInfo.textContent = `...and ${apps.length - limit} more. Refine search.`;
        container.appendChild(moreInfo);
    }
}

async function addExclusion(uid, name) {
    const current = await exec(`grep "^${uid}$" ${FILES.exclusions}`);
    if (current.stdout.trim().length > 0) {
        showToast(`${name} is already blocked!`);
        return;
    }
    await exec(`echo "${uid}" >> ${FILES.exclusions}`);
    await exec(`${NM_BIN} block ${uid}`);
    showToast(`Blocked: ${name}`);
    loadExclusions();
}

async function loadOptions() {
    const swVerbose = document.getElementById('setting-verbose');
    const swSafe = document.getElementById('setting-safemode');
    const btnClear = document.getElementById('btn-clear-rules');

    const checkVerbose = await exec(`[ -f ${FILES.verbose} ] && echo yes`);
    swVerbose.selected = checkVerbose.stdout.includes('yes');

    const checkSafe = await exec(`[ -f ${FILES.disable} ] && echo yes`);
    swSafe.selected = checkSafe.stdout.includes('yes');

    swVerbose.onchange = async () => {
        if (swVerbose.selected) await exec(`touch ${FILES.verbose}`);
        else await exec(`rm ${FILES.verbose}`);
    };

    swSafe.onchange = async () => {
        if (swSafe.selected) await exec(`touch ${FILES.disable}`);
        else await exec(`rm ${FILES.disable}`);
    };

    btnClear.onclick = async () => {
        await exec(`${NM_BIN} clear`);
        showToast("All rules cleared!");
    };
}

document.addEventListener('DOMContentLoaded', () => {
    loadExclusions();
    loadModules();
    initNavigation();
    loadHome();
    document.getElementById('fab-add-exclusion').addEventListener('click', openAppSelector);
    document.getElementById('btn-close-modal').addEventListener('click', () => {
        document.getElementById('app-selector-modal').classList.remove('active');
    });
    document.querySelector('.nav-item.active').click();
});
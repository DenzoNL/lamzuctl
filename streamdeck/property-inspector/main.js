// Lamzu Stream Deck Property Inspector

let websocket = null;
let uuid = null;
let actionInfo = null;
let settings = {};

// Called by Stream Deck when PI loads
function connectElgatoStreamDeckSocket(port, pluginUUID, registerEvent, info, action) {
    uuid = pluginUUID;
    actionInfo = JSON.parse(action);
    settings = actionInfo.payload.settings || {};

    // Connect to Stream Deck
    websocket = new WebSocket(`ws://127.0.0.1:${port}`);

    websocket.onopen = () => {
        // Register property inspector
        websocket.send(JSON.stringify({
            event: registerEvent,
            uuid: uuid
        }));

        // Initialize UI with current settings
        updateUI();

        // Request device list and state from plugin
        sendToPlugin({ command: 'getDevices' });
        sendToPlugin({ command: 'getState' });
    };

    websocket.onmessage = (evt) => {
        const data = JSON.parse(evt.data);

        switch (data.event) {
            case 'didReceiveSettings':
                settings = data.payload.settings || {};
                updateUI();
                break;

            case 'sendToPropertyInspector':
                handlePluginMessage(data.payload);
                break;
        }
    };

    websocket.onerror = (evt) => {
        console.error('WebSocket error:', evt);
    };

    websocket.onclose = () => {
        console.log('WebSocket closed');
    };
}

// Handle messages from the plugin
function handlePluginMessage(payload) {
    switch (payload.event) {
        case 'deviceList':
            populateDeviceSelector(payload.devices);
            break;

        case 'deviceState':
            updateStatus(payload);
            break;
    }
}

// Populate device selector dropdown
function populateDeviceSelector(devices) {
    const select = document.getElementById('deviceSelector');
    const currentValue = select.value;

    // Clear existing options except auto-detect
    while (select.options.length > 1) {
        select.remove(1);
    }

    // Add devices
    devices.forEach((device, index) => {
        const opt = document.createElement('option');
        opt.value = (index + 1).toString(); // 1-based index
        opt.textContent = device.name || `Device ${index + 1}`;
        select.appendChild(opt);
    });

    // Restore selection
    if (currentValue) {
        select.value = currentValue;
    }
}

// Update status display
function updateStatus(state) {
    const statusEl = document.getElementById('status');
    if (state.connected) {
        statusEl.innerHTML = `<span class="status-connected">${state.deviceName}</span>` +
            `<br>Profile: ${state.profile}, DPI Stage: ${state.dpiStage}` +
            `<br>Battery: ${state.battery}%${state.charging ? ' (charging)' : ''}`;
    } else {
        statusEl.innerHTML = '<span class="status-disconnected">Not connected</span>';
    }
}

// Update UI from settings
function updateUI() {
    // Set mode
    const modeEl = document.getElementById('mode');
    modeEl.value = settings.mode || 'battery';

    // Update visibility of profile/DPI containers
    updateContainerVisibility();

    // Set checkbox states based on selectedValues (only for profile mode)
    const selectedValues = settings.selectedValues || [];

    // Update profile checkboxes
    for (let i = 1; i <= 5; i++) {
        const checkbox = document.getElementById(`profile${i}`);
        if (checkbox) {
            checkbox.checked = selectedValues.includes(i);
        }
    }

    // Set showBattery checkbox
    const showBatteryEl = document.getElementById('showBattery');
    if (showBatteryEl) {
        showBatteryEl.checked = settings.showBattery || false;
    }

    // Set device selector
    if (settings.deviceSelector) {
        document.getElementById('deviceSelector').value = settings.deviceSelector;
    }
}

// Update container visibility based on mode
function updateContainerVisibility() {
    const mode = document.getElementById('mode').value;
    const profilesContainer = document.getElementById('profiles-container');
    const dpiInfo = document.getElementById('dpi-info');
    const batteryToggleContainer = document.getElementById('battery-toggle-container');

    // Show profiles for profile mode, info text for DPI mode, hide both for battery
    profilesContainer.classList.toggle('hidden', mode !== 'profile');
    dpiInfo.classList.toggle('hidden', mode !== 'dpi');

    // Show battery toggle for profile and DPI modes (not for battery mode itself)
    batteryToggleContainer.classList.toggle('hidden', mode === 'battery');
}

// Get selected values from checkboxes (only used for profile mode)
function getSelectedValues() {
    const mode = document.getElementById('mode').value;
    const values = [];

    if (mode === 'profile') {
        for (let i = 1; i <= 5; i++) {
            const checkbox = document.getElementById(`profile${i}`);
            if (checkbox && checkbox.checked) {
                values.push(i);
            }
        }
    }
    // DPI mode doesn't need selected values - it cycles through all available stages

    return values;
}

// Send message to plugin
function sendToPlugin(payload) {
    if (websocket && websocket.readyState === WebSocket.OPEN) {
        websocket.send(JSON.stringify({
            event: 'sendToPlugin',
            action: actionInfo.action,
            context: uuid,
            payload: payload
        }));
    }
}

// Save settings to Stream Deck
function saveSettings() {
    const mode = document.getElementById('mode').value;
    const deviceSelector = document.getElementById('deviceSelector').value;
    const selectedValues = getSelectedValues();
    const showBattery = document.getElementById('showBattery').checked;

    settings = {
        mode: mode,
        selectedValues: selectedValues,
        deviceSelector: deviceSelector || null,
        showBattery: showBattery
    };

    if (websocket && websocket.readyState === WebSocket.OPEN) {
        websocket.send(JSON.stringify({
            event: 'setSettings',
            context: uuid,
            payload: settings
        }));
    }
}

// Event listeners
document.addEventListener('DOMContentLoaded', () => {
    // Mode change handler
    document.getElementById('mode').addEventListener('change', () => {
        updateContainerVisibility();
        // Clear checkboxes when changing mode
        clearCheckboxes();
        saveSettings();
    });

    // Profile checkbox listeners
    for (let i = 1; i <= 5; i++) {
        const checkbox = document.getElementById(`profile${i}`);
        if (checkbox) {
            checkbox.addEventListener('change', saveSettings);
        }
    }

    // Battery toggle listener
    document.getElementById('showBattery').addEventListener('change', saveSettings);

    document.getElementById('deviceSelector').addEventListener('change', saveSettings);

    document.getElementById('refreshBtn').addEventListener('click', () => {
        sendToPlugin({ command: 'getDevices' });
        sendToPlugin({ command: 'getState' });
    });
});

// Clear profile checkboxes
function clearCheckboxes() {
    for (let i = 1; i <= 5; i++) {
        const checkbox = document.getElementById(`profile${i}`);
        if (checkbox) checkbox.checked = false;
    }
}

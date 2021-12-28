# Guard working scheme specification
## Device specs
### ESP32 board:
Pinout possibilities summary: 
We assume 4 miners to serve for now:
Input only pins {34,35,36,39} we will use as LED input
Available power/reset/LED sets:
* 25/26/36 - pins set index 0
* 16/17/39 - pins set index 1
* 19/21/34 - pins set index 2
* 23/22/35 - pins set index 3

## Initialization of a device:
Steps:
1. Obtain Ethernet/WiFi connection.
2. Connect to MQTT the server:
    1. Subscribe guard config and control topic.
    2. Publish guard started message.
    3. Wait for a configuration from the server. There must be delivered information like: miners names, pinouts.
3. Set pinout of each miner.
4. Check miners state.
5. Subscribe command control topics for all miners.
6. Unsubscribe config topic.
7. Publish information that initialization is done to the server.
8. Guard runs control loop.

## Control loop:
Steps:
1. Runs MQTT client loop.
2. In case we use WiFi run WLAN setup loop.
3. Check we obtain any control message, if true then:
    1. Check message type and start appropriate operating procedure.
    2. If needed, set execution timers.
4. Check any command progress:
    1. Check appropriate procedure succeed or failed.
    2. Publish message with report to the server.
5. Check miners state:
    1. If state is desirable do nothing.
    2. Else handle it and publish error message.

### States description:
- `NotDefined` - Any of below.
- `PoweredOff` - Miner is power off, from this state miner can start work.
- `Starting` - Miner is starting, if succed state can be changed to `Running`.
- `Running` - Miner started and works correctly, from this state can be run power off/restart.
- `Stopping` - Miner is powering off.
- `HardStopping` - Miner is stopping by pin power (5 sec.).
- `Restarting` - Pin restart was used, then it started and state can be change to `Running`. Otherwise we have undesirable behaviour, action must be taken.
- `HardRestarting` - Pin power was used for 5 sec., then it started and state can be change to `Running`. Otherwise we have undesirable behaviour, action must be taken.
- `Aborted` - Miner was running but unexpectedly pin LED id LOW. We need to report that fact and wait for command.
- `Unreachable` - Miner doesn't react on commands properly.

### Commands description:
- `NotDefined` - Any of below.
- `Idle` - Miner waits for commnd potentially. 
- `PowerOn` - Power up miner.
- `PowerOff` - Power down miner.
- `HardStop` - Power down miner by pin power (5 sec.).
- `Reset` - Restart miner with pin reset.
- `HardReset` - Restert miner by pin power (5 sec.)
- `LedReport` - Report to server state of pin LED.


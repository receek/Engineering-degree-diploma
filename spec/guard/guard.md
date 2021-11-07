# Guard working scheme specification
## Device specs
### ESP32 board:
Pinout possibilities summary: 

## Initialization of a device:
Steps:
1. Obtain Ethernet/WiFi connection.
2. Connect to MQTT the server:
    1. Subscribe guard config and control topic.
    2. Publish guard started topic.
    3. Wait for configuration from the server. There must be delivered information like: miners ids, pinouts
    4. Subscribe power/reset/LED/state control topics of each miner.
3. Set pinout of each miner.
4. Check all miners states.
5. Unsubscribe config topic.
6. Publish information that initialization is done to the server.
7. Guard runs control loop.

## Control loop:
Steps:
1. Runs MQTT client loop.
2. In case we use WiFi run WLAN setup loop.
3. Check we obtain any control message, if true then:
    1. Check message type and start appropriate operating procedure.
    2. If needed, set execution timers.
4. Check if any execution timers is done, if true then:
    1. Check appropriate procedure succeed or failed.
    2. Publish message with report to the server.
5. Check miners state:
    1. If state is desirable do nothing.
    2. Else publish error message.

## Miner struct/class fields:
* `name`
* `state` {Not_defined, Stopped, Hard_Stopped, Starting, Running, Soft_Restarting, Hard_Restarting, Aborted, Unreachable}
* `pinout` (x3)
* `fails_counter`

### States description:
- `Not_Defined` - any of below.
- `Stopped` - miner is power off, from this state miner can start work
- `Hard_Stopped` - miner is stopped by 5 sec. pin power.
- `Starting` - miner is starting, we need from server message that miner consume energy, then it started and state can be change to `Running`.
- `Running` - miner started and works correctly, from this state can be power off/restarted.
- `Stopping` - miner is powering off.
- `Soft_Restarting` - pin restart was used, we wait for messege that miner consume energy, then it started and state can be change to `Running`. Otherwise we have undesirable behaviour, we must take action.
- `Hard_Restarting` - pin power was used for 5 sec., we wait for messege that miner consume energy, then it started and state can be change to `Running`. Otherwise we have undesirable behaviour, we must take action.
- `Aborted` - Miner was starting/running but unexpectedly shutdown. We need to report that fact and wait for command.
- `Unreachable` - Miner doesn't react on commands.


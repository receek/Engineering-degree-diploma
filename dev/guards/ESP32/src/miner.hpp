#ifndef MINER_H
#define MINER_H

#include <MQTT.h>

#include "globals.hpp"
#include "timer_wrapper.hpp"
#include "utils.hpp"

enum class State {
    NotDefined = 0,
    PoweredOff,
    HardStopped,
    Starting,
    Running,
    Stopping, 
    Restarting,
    HardRestarting,
    Aborted,
    Unreachable
};

const char * getStateName(State);

enum class Command {
    NotDefined = 0,
    Idle,
    PowerOn,
    PowerOff,
    HardStop,
    SoftReset,
    HardReset,
    LedReport
};

Command getCommandFromName(String&);

class Miner {
private:

public:
    static TimerWrapper& timer;
    static MQTTClient * client;
    
    u8 pinPower;
    u8 pinReset;
    u8 pinLed;

    String name;
    String commandPrefixTopic;
    String errorLogTopic;
    
    State state;
    
    Command command;
    bool isCommandRunning;
    u64 timestamp;
    u32 commandStage = 0;

    void setConfiguration(u8, String&);
    void runCommand();
    void watchCommandExecution();
    void watchMinerState();
};

#endif
#ifndef MINER_H
#define MINER_H

#include <MQTT.h>

#include "globals.hpp"
#include "timer_wrapper.hpp"
#include "utils.hpp"

enum class State {
    NotDefined = 0,
    PoweredOff,
    Starting,
    Running,
    Stopping,
    HardStopping, 
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
    Reset,
    HardReset,
    LedReport
};

const char * getCommandName(Command);
Command getCommandFromName(String&);

class Miner {
private:
    static TimerWrapper& timer;

    u64 timestamp;
    u32 commandStage = 0;

    String commandPrefixTopic;
    String errorLogTopic;

public:
    static MQTTClient * client;

    u8 pinSet;
    u8 pinPower;
    u8 pinReset;
    u8 pinLed;

    String id;

    State state;
    
    Command command;
    bool isCommandRunning;

    void setConfiguration(u8, String&);
    void runCommand();
    void watchCommandExecution();
    void watchMinerState();
};

#endif
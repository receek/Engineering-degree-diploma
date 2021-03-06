#ifndef MINER_H
#define MINER_H

#include <MQTT.h>

#include "globals.hpp"
#include "timer_wrapper.hpp"
#include "utils.hpp"

enum class State {
    Undefined = 0,
    PoweredOff = 1,
    Starting = 2,
    Running = 3,
    Stopping = 4,
    HardStopping = 5, 
    Restarting = 6,
    HardRestarting = 7,
    Aborted = 8,
    Unreachable = 9
};

const char * getStateName(State);

enum class Command {
    Undefined = 0,
    Idle = 1,
    PowerOn = 2,
    PowerOff = 3,
    HardStop = 4,
    Reset = 5,
    HardReset = 6,
    StateReport = 7
};

const char * getCommandName(Command);
Command getCommandFromName(String&);

class Miner {
private:
    static TimerWrapper& timer;

    u64 timestamp;
    u32 commandStage = 0;

public:
    String alertTopic;
    String commandTopic;
    String stateTopic;

    static MQTTClient * client;

    u8 pinSet;
    u8 pinPower;
    u8 pinReset;
    u8 pinLed;

    String id;

    State state;
    
    Command command;
    bool isCommandRunning;
    bool stateToReport;

    bool busyToPublish;
    bool undefinedToPublish;

    void setConfiguration(u8, String&);
    void runCommand();
    void watchCommandExecution();
    void watchMinerState();
    void sendStateMessage();
    void sendCommandBusy();
    void sendCommandUndefined();
};

#endif
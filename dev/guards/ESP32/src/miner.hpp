#ifndef MINER_H
#define MINER_H

#include "globals.hpp"
#include "utils.hpp"

enum State {
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

class Miner {
private:

public:
    u8 pinPower;
    u8 pinReset;
    u8 pinLed;

    State state;
    String name;

    void setConfiguration(u8, String&);
};

#endif
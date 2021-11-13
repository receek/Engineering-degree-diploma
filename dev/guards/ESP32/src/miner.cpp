
#include "miner.hpp"

static const char * stateNames[] = {
    "NotDefined",
    "PoweredOff",
    "HardStopped",
    "Starting",
    "Running",
    "Stopping", 
    "Restarting",
    "HardRestarting",
    "Aborted",
    "Unreachable"
};

const char * getStateName(State state) {
    return stateNames[static_cast<u32>(state)];
}

void Miner::setConfiguration(u8 pinSet, String & name_) {
    pinPower = PINOUTS_SET[pinSet][0];
    pinReset = PINOUTS_SET[pinSet][1];
    pinLed = PINOUTS_SET[pinSet][2];

    state = NotDefined;
    name = name_;
}

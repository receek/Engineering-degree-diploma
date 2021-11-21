
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

static const char * commandNames[] = {
    "NotDefined",
    "Idle",
    "PowerOn",
    "PowerOff",
    "HardStop",
    "SoftReset",
    "HardReset",
    "LedReport"
};

const char * getCommandName(Command command) {
    return commandNames[static_cast<u32>(command)];
}

Command getCommandFromName(String& state) {
    for (u32 i = 1; i < (sizeof(commandNames) / sizeof(commandNames[0])); ++i) {
        if (state == commandNames[i])
            return static_cast<Command>(i);
    }
    return Command::NotDefined;
}

TimerWrapper& Miner::timer = getTimerInstance();
MQTTClient *Miner::client = 0;

void Miner::setConfiguration(u8 pinSet, String & name_) {
    pinPower = PINOUTS_SET[pinSet][0];
    pinReset = PINOUTS_SET[pinSet][1];
    pinLed = PINOUTS_SET[pinSet][2];

    name = name_;
    commandPrefixTopic = GUARD_PREFIX_TOPIC + "miner/" + name + "/";
    errorLogTopic = GUARD_PREFIX_TOPIC + "miner/" + name + "/error";

    state = State::NotDefined;
    command = Command::Idle;
    isCommandRunning = false;
    timestamp = 0;
    commandStage = 0;
}

void Miner::runCommand() {
    switch (command)
    {
    case Command::NotDefined:
    

        break;
    case Command::Idle:
        
        break;
    case Command::PowerOn: {
        if (state == State::PoweredOff || state == State::Aborted || state == State::Unreachable) {
            timestamp = timer.getTimestamp();
            digitalWrite(pinPower, LOW);
            state = State::Starting;
        } else {
            client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Wrong state!");
        }
    }
    break;
    case Command::PowerOff: {
        if (state == State::Running || state == State::Unreachable) {
            timestamp = timer.getTimestamp();
            digitalWrite(pinPower, LOW);
            state = State::Stopping;
        } else {
            client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Wrong state!");
        }
    }
    break;
    case Command::HardStop:
    
        break;
    case Command::SoftReset:
    
        break;
    case Command::HardReset:
    
        break;
    case Command::LedReport:
    
        break;
    default:
        /* Report error message */
        break;
    }

    isCommandRunning = true;
    commandStage = 0;
}

void Miner::watchCommandExecution() {
    switch (command)
    {
        case Command::NotDefined: {
        
            break;
        }
        case Command::Idle:
            
            break;
        case Command::PowerOn: {
            if (commandStage == 0) {
                if (timer.isTimeElapsed(timestamp, 1500)) {
                    digitalWrite(pinPower, HIGH);
                    timestamp = timer.getTimestamp();
                    ++commandStage;
                }
            } else if (timer.isTimeElapsed(timestamp, 5000)) {
                if (digitalRead(pinLed) == HIGH) {
                    /* Send command execution DONE */
                    client->publish(commandPrefixTopic + getCommandName(command), "DONE");
                    
                    state = State::Running;
                } else {
                    /* Send command execution FAILED */
                    client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Miner is unreachable");
                    
                    state = State::Unreachable;
                }
                command = Command::Idle;
                isCommandRunning = false;
            }
        }
        break;
        case Command::PowerOff: {
            if (commandStage == 0) {
                if (timer.isTimeElapsed(timestamp, 1500)) {
                    digitalWrite(pinPower, HIGH);
                    timestamp = timer.getTimestamp();
                    ++commandStage;
                }
            } else if (digitalRead(pinLed) == LOW) {
                /* Send command execution DONE */
                client->publish(commandPrefixTopic + getCommandName(command), "DONE");
                
                command = Command::Idle;
                isCommandRunning = false;
                state = State::PoweredOff;
            } else if (timer.isTimeElapsed(timestamp, 120000)) {
                /* Send command execution FAILED */
                client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Miner is unreachable");
                
                command = Command::Idle;
                isCommandRunning = false;
                state = State::Unreachable;
            }
        }
        break;
        case Command::HardStop: {
        
        }
        break;
        case Command::SoftReset: {
        
            break;
        }
        case Command::HardReset: {
        
            break;
        }
        case Command::LedReport: {
            int ledState = digitalRead(pinLed);
            if (ledState == HIGH) {
                client->publish(commandPrefixTopic + getCommandName(command), "ON");
            }
            else {
                client->publish(commandPrefixTopic + getCommandName(command), "OFF");
            }
            command = Command::Idle;
            isCommandRunning = false;
        }
        break;
        default: {
            /* Report error message */
            break;
        }
    }
}

void Miner::watchMinerState() {
    if (isCommandRunning)
        return;

    switch (state)
    {
    case State::NotDefined:
    
        break;
    case State::PoweredOff:
    case State::HardStopped: {
        if (digitalRead(pinLed) == HIGH) {
            state = State::Unreachable;

            /* Report problem  */
            client->publish(errorLogTopic, "[PoweredOff][ON]");
        }
    }
    break;
    case State::Starting:
    
        break;
    case State::Running: {
        if (digitalRead(pinLed) == LOW) {
            state = State::Aborted;

            /* Report problem  */
            client->publish(errorLogTopic, "[Running][OFF]");
        }
    }
    break;
    case State::Stopping: {
        
    }
        break;
    case State::Restarting:
    
        break;
    case State::Aborted:
    
        break;
    case State::Unreachable:
        
        break;
    default:
        /* Report error message */
        break;
    }
}

/* ESP32 board */
#include <Arduino.h>

#include <WiFi.h>
#include <MQTT.h>

#include "src/miner.hpp"
#include "src/globals.hpp"
#include "src/timer_wrapper.hpp"
#include "src/utils.hpp"

static WiFiClient network;
static MQTTClient client;


static Miner miners[MAX_MINERS];
static i32 minersCount = -1;

static char response[128];

TimerWrapper& timer = getTimerInstance();
u64 timestamp = 0;
u64 pingTimestamp = 0;

bool isConfigSet = false;
bool runRestart = false;
bool runAnnounce = false;

void connectMQTTClient() {
    Serial.println("MQTT Client connecting...");
    
    while (WiFi.status() != WL_CONNECTED) {
        Serial.print(".");
        delay(500);
    }

    Serial.println("WiFi connected!");

    while (!client.connect(DEV_ID, USER, PASSWORD)) {
        delay(1000);
    }

    Serial.println("Connected!");
}


void applyConfig() {
    for (i32 i = 0; i < minersCount; ++i) {
        /* Set pinouts */
        digitalWrite(miners[i].pinPower, HIGH);
        digitalWrite(miners[i].pinReset, HIGH);
        pinMode(miners[i].pinPower, OUTPUT);
        pinMode(miners[i].pinReset, OUTPUT);
        pinMode(miners[i].pinLed, INPUT);

        /* Check miner state */
        delay(100);
        miners[i].state = digitalRead(miners[i].pinLed) == HIGH ?
            State::Running :
            State::PoweredOff;

    }

    Miner::client = &client;

    /* Subscribe miner control topics */
    for(int i = 0; i < minersCount; i++) {
        client.subscribe(GUARD_PREFIX_TOPIC + "miners/" + miners[i].id);
    }
    client.subscribe(GUARD_PREFIX_TOPIC + "command");
    client.subscribe(GUARD_COMMAND_TOPIC);
}

void parseConfig(String &payload) {
    /* 
    Assume that message format is:
    "miners_count miner0_name miner0_pinset miner1_name miner1_pinset ... "
    */

    u32 from = 0;
    u32 to = 0;
    u32 length = payload.length();

    u8 pinset;
    String name;

    while (payload[to] != ' ')
      to++;
    minersCount = payload.substring(from, to).toInt();
    
    from = to = to + 1;
    
    for (i32 i = 0; i < minersCount; ++i) {
        while (payload[to] != ' ') 
            to++;
        name = payload.substring(from, to);

        from = to = to + 1;
        while (to < length && payload[to] != ' ')
            to++;
        pinset = payload.substring(from, to).toInt();

        miners[i].setConfiguration(pinset, name);
    }

    isConfigSet = true;
}

i32 getMinerIndex(String& minerName) {
    for (i32 i = 0; i < minersCount; ++i) {
        if (miners[i].id == minerName)
            return i;
    }
    return -1;
}

void startUpMessageReceiver(String &topic, String &payload) {
    Serial.println("incoming: " + topic + " = " + payload);

    if (topic == GUARD_CONFIG_TOPIC) {
        parseConfig(payload);
    } else {
        Serial.println("ERROR: Undefined topic arrived!");
        Serial.printf("Topic: %s\nPayload: %s\n", topic, payload);
    }
}

void controlMessageReceiver(String &topic, String &payload) {
    Serial.println("incoming: " + topic + " = " + payload);
    Serial.flush();

    if (topic.startsWith(GUARD_PREFIX_TOPIC)) {
        String subtopic = topic.substring(GUARD_PREFIX_TOPIC.length());

        if (subtopic.startsWith("miners/")) {
            /* Received miner control message */
            subtopic = subtopic.substring(7);

            i32 id = getMinerIndex(subtopic);

            if(id < 0) {
                // client.publish(topic, "FAILED");
                // Serial
                return;
            }

            Command command = getCommandFromName(payload);

            if (command == Command::Undefined) {
                /* Undefined command received */
                sprintf(response, "command=UNDEFINED, state=%s", miners[id].state);
                client.publish(miners[id].commandTopic, response);
                return;
            } else if (command == Command::StateReport) {
                /* Set flag */
                miners[id].statusToReport = true;
                return;
            } else if (miners[id].command != Command::Idle) {
                /* Miner should be in idle command mode */
                sprintf(response, "command=BUSY, state=%s", miners[id].state);
                client.publish(topic + "/command", response);
                return;
            }

            miners[id].command = command;
        }
        else if (subtopic.startsWith("command") && payload.startsWith("reset")) {
            /* Restart guard */
            runRestart = true;
        }
        else {
            /* Undefined topic */
            // client.publish(GUARD_ERROR_LOG_TOPIC, String("Undefined topic: ") + topic);
            Serial.printf("Guard received message from unspecified topic: %s\n", topic);
            Serial.flush();
        }
    } else if (topic.startsWith(GUARD_COMMAND_TOPIC)) {
        if (payload.startsWith("announce")) {
            runAnnounce = true;
        } else if (payload.startsWith("reset")) {
            runRestart = true;
        }
    }
    else {
        /* Undefined message */
        Serial.printf("Guard received message from unspecified topic: %s\n", topic);
        Serial.flush();
    }
}

void printConfigSummary() {
    Serial.printf("Avaible miners under guard control: %d\n", minersCount);
    for (u32 i = 0; i < minersCount; ++i) {
        Serial.printf("Miner %d details:\n", i);
        Serial.printf("Name: %s\n", miners[i].id);
        Serial.printf("Pinouts:  power=%d, reset=%d, led=%d\n", 
            miners[i].pinPower, miners[i].pinReset, miners[i].pinLed);
        Serial.printf("State: %s\n", getStateName(miners[i].state));
        Serial.flush();
    }
}

void runGuardRestart() {
    client.publish(GUARD_PREFIX_TOPIC + "reset", "RESTARTING");

    Serial.println("Guard runs restart!");
    Serial.flush();

    delay(100);
    ESP.restart();
}

void runGuardAnnounce() {
    char buffer[256];
    char miner[128];
    String minersBuffer;

    minersBuffer += "[";
    for(int i = 0; i < minersCount; i++) {
        sprintf(miner, "{\"id\": \"%s\", \"pinset\": %d}", miners[i].id, miners[i].pinSet);
        minersBuffer += miner;
        if (i < minersCount - 1) {
            minersBuffer += ',';
        }
    }
    minersBuffer += "]";

    sprintf(buffer, 
        "{\"id\": \"%s\", \"type\": \"%s\", \"miners\": %s}", 
        DEV_ID, DEV_TYPE, minersBuffer.c_str()
    );

    Serial.println("Guard publishs announce message!");
    Serial.flush();

    client.publish(GUARD_ANNOUNCE_TOPIC, buffer);
}

void runGuardPing() {
    client.publish(GUARD_PING_TOPIC);
}

void setup() {
    Serial.begin(115200);
    Serial.flush();

    /* WiFi configuring */
    WiFi.begin(WIFI_SSID, WIFI_PASSWORD);

    /* MQTT connecting */
    client.begin(IP_MQTT_BROKER, PORT_MQTT_BROKER, network);
    client.onMessage(startUpMessageReceiver);
    connectMQTTClient();

    client.subscribe(GUARD_CONFIG_TOPIC);
    client.publish(GUARD_STARTED_TOPIC, DEV_ID);
    timestamp = timer.getTimestamp();

    /* Waiting for ports configset message */
    while (!isConfigSet) {
        if (timer.isTimeElapsed(timestamp, 10000)){
            /* Republishing config request */
            client.publish(GUARD_STARTED_TOPIC, DEV_ID);
            timestamp = timer.getTimestamp();
        }

        client.loop();
        delay(200);
    }

    client.unsubscribe(GUARD_CONFIG_TOPIC);

    /* Set pinout and subscribe all needed topics */
    applyConfig();

    /* Change message handler */
    client.onMessage(controlMessageReceiver);

    /* Publish guard is configured */
    client.publish(GUARD_CONFIGURED_TOPIC, DEV_ID);

    printConfigSummary();
    timestamp = pingTimestamp = timer.getTimestamp();
}

void loop() {
    client.loop();
    delay(10);

    if (!client.connected()) {
        connectMQTTClient();
    }
    
    if (runRestart) {
        runGuardRestart();
    }

    if (runAnnounce) {
        runAnnounce = false;
        runGuardAnnounce();
    }

    /* Checking do miners need report state */
    for (u32 i = 0; i < minersCount; ++i) {
        if (miners[i].statusToReport) {
            miners[i].sendStatusMessage();
        }
    }

    /* Checking commands on each miner */
    for (u32 i = 0; i < minersCount; ++i) {
        if (miners[i].command > Command::Idle && !miners[i].isCommandRunning) {
            /* Run command on miner  */
            miners[i].runCommand();
        }
    }

    /* Checking execution timers */
    for (u32 i = 0; i < minersCount; ++i) {
        if (miners[i].command > Command::Idle && miners[i].isCommandRunning) {
            /* Check command execution */
            miners[i].watchCommandExecution();
        }
    }

    if (timer.isTimeElapsed(timestamp, 500)) {
        for (u32 i = 0; i < minersCount; ++i) {
            miners[i].watchMinerState();
        }

        timestamp = timer.getTimestamp();
    }

    if (timer.isTimeElapsed(pingTimestamp, 30000)) {
        runGuardPing();
        pingTimestamp = timer.getTimestamp();
    }
}
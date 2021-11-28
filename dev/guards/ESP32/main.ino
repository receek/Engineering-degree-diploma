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

TimerWrapper& timer = getTimerInstance();
u64 timestamp = 0;

bool isConfigSet = false;
bool runRestart = false;

void connectMQTTClient() {
    Serial.println("MQTT Client connecting...");
    
    while (WiFi.status() != WL_CONNECTED) {
        Serial.print(".");
        delay(500);
    }

    Serial.println("WiFi connected!");

    while (!client.connect(DEV_NAME, USER, PASSWORD)) {
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
    client.subscribe(GUARD_PREFIX_TOPIC + "miners/+");
    client.subscribe(GUARD_PREFIX_TOPIC + "reset");
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
        if (miners[i].name == minerName)
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

    if (topic.startsWith(GUARD_PREFIX_TOPIC)) {
        String subtopic = topic.substring(GUARD_PREFIX_TOPIC.length());

        if (subtopic.startsWith("miners/")) {
            /* Received miner control message */
            subtopic = subtopic.substring(7);

            i32 id = getMinerIndex(subtopic);

            if(id < 0) {
                client.publish(topic, "FAILED: Undefined miner name");
                return;
            }

            Command command = getCommandFromName(payload);

            if (command == Command::NotDefined) {
                /* Undefined command received */
                client.publish(topic, "FAILED: Undefined command");
                return;
            } else if (miners[id].command != Command::Idle) {
                /* Miner should be in idle command mode */
                client.publish(topic, "FAILED: Miner is busy");
                return;
            }

            miners[id].command = command;
        }
        else if (subtopic.startsWith("reset")) {
            /* Restart guard */
            runRestart = true;
        }
        else {
            /* Undefined topic */
            client.publish(GUARD_ERROR_LOG_TOPIC, String("Undefined topic: ") + topic);
            Serial.printf("Guard received message from unspecified topic: %s\n", topic);
            Serial.flush();
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
        Serial.printf("Name: %s\n", miners[i].name);
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

void setup() {
    Serial.begin(115200);

    /* WiFi configuring */
    WiFi.begin(WIFI_SSID, WIFI_PASSWORD);

    /* MQTT connecting */
    client.begin(IP_MQTT_BROKER, PORT_MQTT_BROKER, network);
    client.onMessage(startUpMessageReceiver);
    connectMQTTClient();

    client.subscribe(GUARD_CONFIG_TOPIC);
    client.publish(GUARD_STARTED_TOPIC, DEV_NAME);
    timestamp = timer.getTimestamp();

    /* Waiting for ports configset message */
    while (!isConfigSet) {
        if (timer.isTimeElapsed(timestamp, 10000)){
            /* Republishing config request */
            client.publish(GUARD_STARTED_TOPIC, DEV_NAME);
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
    client.publish(GUARD_CONFIGURED_TOPIC, DEV_NAME);

    printConfigSummary();
    timestamp = timer.getTimestamp();
}

void loop() {
    client.loop();
    delay(10);

    if (runRestart) {
        runGuardRestart();
    }

    if (!client.connected()) {
        connectMQTTClient();
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
}
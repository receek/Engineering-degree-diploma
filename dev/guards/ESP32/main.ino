/* ESP32 board */
#include <Arduino.h>
#include <WiFi.h>
#include <MQTT.h>

#include "timer_wrapper.hpp"
#include "utils.hpp"


const char WIFI_SSID[] = "NETIASPOT-2.4GHz-YC9V";
const char WIFI_PASSWORD[] = "xXjgfYMt";

const IPAddress IP_MQTT_BROKER(192, 168, 100, 5);
const int PORT_MQTT_BROKER = 1883;
const char DEV_NAME[] = "Mega0001";
const char USER[] = "broker";
const char PASSWORD[] = "broker";

const String GUARD_STARTED_TOPIC = String("guards/started");
const String GUARD_CONFIG_TOPIC = String("guards/config/") + DEV_NAME;

const String GUARD_POWER_RUN_TOPIC = String("guards/") + DEV_NAME + String("/power");
const String GUARD_SOFT_RESET_RUN_TOPIC = String("guards/") + DEV_NAME + String("/soft_reset");
const String GUARD_HARD_RESET_RUN_TOPIC = String("guards/") + DEV_NAME + String("/hard_reset");

const String GUARD_LED_REQUEST_TOPIC = String("guards/") + DEV_NAME + String("/LED/request");
const String GUARD_LED_STATE_TOPIC = String("guards/") + DEV_NAME + String("/LED/state");

WiFiClient net;
MQTTClient client;

bool configIsSet = false; 
bool runPower = false;
bool runSoftReset = false;
bool runHardReset = false;
bool ledRequested = false;


const int IN_PIN = 35;
int pinPower;
int pinReset;
int pinLed;

int val = 0;

void connectMQTTClient() {
    TimerWrapper & tttt = getTimerInstance();
    Serial.println("MQTT Client connecting...");
    
    while (WiFi.status() != WL_CONNECTED) {
        Serial.print(".");
        delay(500);
    }

    while (!client.connect(DEV_NAME, USER, PASSWORD)) {
        delay(1000);
    }

    Serial.println("Connected!");
}


void configurePorts() {
    pinMode(pinPower, OUTPUT);
    pinMode(pinReset, OUTPUT);
    pinMode(pinLed, INPUT);

    digitalWrite(pinPower, HIGH);
    digitalWrite(pinReset, HIGH);
}

void parsePorts(String &payload) {
    int from = 0;
    int to = 0;

    while (payload[to] != ' ')
      to++;
    pinPower = payload.substring(from, to).toInt();
    
    from = to = to + 1;
    while (payload[to] != ' ')
      to++;
    pinReset = payload.substring(from, to).toInt();

    from = to + 1;
    pinLed = payload.substring(to).toInt();

    configIsSet = true;
    client.unsubscribe("guards/config/Mega0001");
}

void messageReceived(String &topic, String &payload) {
    Serial.println("incoming: " + topic + " = " + payload);

    if (topic == GUARD_POWER_RUN_TOPIC) {
        runPower = true;
    } else if (topic == GUARD_SOFT_RESET_RUN_TOPIC) {
        runSoftReset = true;
    } else if (topic == GUARD_LED_REQUEST_TOPIC) {
        runHardReset = true;
    } else if (topic == GUARD_LED_REQUEST_TOPIC) {
        ledRequested = true;
    } else if (topic == GUARD_CONFIG_TOPIC) {
        parsePorts(payload);
    }
}

void setup() {
    Serial.begin(115200);

    /* WiFi configuring */
    WiFi.begin(WIFI_SSID, WIFI_PASSWORD);

    /* MQTT connecting */
    client.begin(IP_MQTT_BROKER, PORT_MQTT_BROKER, net);
    client.onMessage(messageReceived);
    connectMQTTClient();

    client.subscribe(GUARD_CONFIG_TOPIC);
    client.subscribe(GUARD_POWER_RUN_TOPIC);
    client.publish(GUARD_STARTED_TOPIC, DEV_NAME); // need republishing messages in case main server is not properly started

    /* Waiting for ports configset message */
    while (!configIsSet) {
        client.loop();
        delay(200);
    }

    configurePorts();

    Serial.println("Ports configured to:");
    Serial.print("Power = ");
    Serial.print(pinPower);
    Serial.print(", reset = ");
    Serial.print(pinReset);
    Serial.print(", LED = ");
    Serial.println(pinLed);
    Serial.flush();

    Serial.println(GUARD_POWER_RUN_TOPIC);

    pinMode(IN_PIN, INPUT);
}

void loop() {
    client.loop();
    delay(10);

    if (!client.connected()) {
        connectMQTTClient();
    }
    
    if (runPower) {
        Serial.println("Power run has been requested");

        digitalWrite(pinPower, LOW);
        delay(1000);
        digitalWrite(pinPower, HIGH);
        
        runPower = false;
    } else if (runSoftReset) {
        Serial.println("Soft reset run has been requested");
        runSoftReset = false;
    } else if (runHardReset) {
        Serial.println("Hard reset run has been requested");
        runHardReset = false;
    } else if (ledRequested) {
        Serial.println("LED state has been requested");
        ledRequested = false;
    }

    val = digitalRead(IN_PIN);
    if (val == HIGH) {
        Serial.println("In is HIGH");
    } else {
        Serial.println("In is LOW");
    }

    delay(100);
}
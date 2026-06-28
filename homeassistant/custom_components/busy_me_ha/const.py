"""Constants for the Busy Me Light integration."""

DOMAIN = "busy_me_ha"
PLATFORMS = ["light"]

DEVICE_MANUFACTURER = "Various (Kuando, Luxafor, ThingM, MuteMe)"

ATTR_EFFECT = "effect"
EFFECT_NONE = "none"
EFFECT_BLINK = "blink"
EFFECT_DIM = "dim"
EFFECT_SLEEP = "sleep"

EFFECT_LIST = [EFFECT_NONE, EFFECT_BLINK, EFFECT_DIM, EFFECT_SLEEP]

SCAN_INTERVAL_SECONDS = 30

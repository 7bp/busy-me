"""Light platform for Busy Me — controls USB busy lights via /dev/hidraw*."""

from __future__ import annotations

import logging
from typing import Any

from homeassistant.components.light import (
    ATTR_EFFECT,
    ATTR_RGB_COLOR,
    ColorMode,
    LightEntity,
)
from homeassistant.config_entries import ConfigEntry
from homeassistant.core import HomeAssistant
from homeassistant.helpers.entity import DeviceInfo
from homeassistant.helpers.entity_platform import AddEntitiesCallback

from .const import (
    DOMAIN,
    EFFECT_BLINK,
    EFFECT_DIM,
    EFFECT_LIST,
    EFFECT_NONE,
    EFFECT_SLEEP,
)
from . import hidraw

_LOGGER = logging.getLogger(__name__)


async def async_setup_platform(
    hass: HomeAssistant,
    config: dict[str, Any],
    async_add_entities: AddEntitiesCallback,
    discovery_info: dict[str, Any] | None = None,
) -> None:
    """Set up from configuration.yaml."""
    await _add_devices(async_add_entities)


async def async_setup_entry(
    hass: HomeAssistant,
    entry: ConfigEntry,
    async_add_entities: AddEntitiesCallback,
) -> None:
    """Set up from a config entry (UI flow)."""
    await _add_devices(async_add_entities, entry.options)


async def _add_devices(
    async_add_entities: AddEntitiesCallback,
    options: dict | None = None,
) -> None:
    """Scan for connected busylights and create entities."""
    devices = await _run_sync(hidraw.scan)
    if not devices:
        _LOGGER.warning(
            "No busylight devices found. Check that /dev/hidraw* exists "
            "and the container user has read/write permission."
        )
        return

    entities = [BusyMeLight(d, options) for d in devices]
    async_add_entities(entities)
    _LOGGER.info("Found %d busy light(s): %s", len(entities),
                  ", ".join(e.name for e in entities))


async def _run_sync(fn, *args):
    """Run a blocking function in the default executor."""
    import asyncio
    return await asyncio.get_running_loop().run_in_executor(None, fn, *args)


class BusyMeLight(LightEntity):
    """A USB busylight exposed as a Home Assistant light entity."""

    _attr_has_entity_name = True
    _attr_color_mode = ColorMode.RGB
    _attr_supported_color_modes = {ColorMode.RGB}
    _attr_effect_list = EFFECT_LIST

    def __init__(self, dev: dict, options: dict | None = None) -> None:
        """Wrap a discovered hidraw device into a HA light."""
        self._dev = dev
        self._options = options or {}
        self._protocol = dev["protocol"]
        self._is_muteme = dev["protocol"] == "muteme"

        serial = dev.get("serial") or dev["path"]
        self._attr_unique_id = f"busy_me_ha_{serial}"
        self._attr_name = dev["name"]
        self._attr_effect = self._options.get("default_effect", EFFECT_NONE)
        self._attr_rgb_color = (255, 255, 255)
        self._attr_is_on = False
        self._attr_device_info = DeviceInfo(
            identifiers={(DOMAIN, serial)},
            name=self._attr_name,
            manufacturer="Various",
            model=dev["name"],
            sw_version=None,
        )

    async def async_turn_on(self, **kwargs: Any) -> None:
        """Turn on with optional RGB colour and/or effect."""
        r, g, b = kwargs.get(ATTR_RGB_COLOR, self._attr_rgb_color or (255, 255, 255))
        effect = kwargs.get(ATTR_EFFECT, self._attr_effect)
        path = self._dev["path"]

        def _do():
            if self._is_muteme:
                blink = effect == EFFECT_BLINK
                dim = effect == EFFECT_DIM
                sleep = effect == EFFECT_SLEEP
                hidraw.send_muteme_with_effect(path, r, g, b, blink, dim, sleep)
            elif self._protocol == "kuando":
                hidraw.send_kuando(path, r, g, b)
            elif self._protocol == "luxafor":
                hidraw.send_luxafor(path, r, g, b)
            elif self._protocol == "blink1":
                hidraw.send_blink1(path, r, g, b)

        await _run_sync(_do)

        self._attr_rgb_color = (r, g, b)
        self._attr_effect = effect
        self._attr_is_on = True
        self.async_write_ha_state()

    async def async_turn_off(self, **kwargs: Any) -> None:
        """Turn off."""
        path, protocol = self._dev["path"], self._protocol

        def _do():
            hidraw.send_off(path, protocol)

        await _run_sync(_do)
        self._attr_is_on = False
        self.async_write_ha_state()

    async def async_update(self) -> None:
        """Check device connectivity (device file still exists)."""
        self._attr_available = self._dev["path"] in (
            await _run_sync(self._list_hidraw)
        )

    @staticmethod
    def _list_hidraw() -> set[str]:
        return set(hidraw._hidraw_paths())

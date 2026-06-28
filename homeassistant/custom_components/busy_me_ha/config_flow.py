"""Config and options flow for Busy Me Light integration."""

from __future__ import annotations

import logging
from typing import Any

import voluptuous as vol

from homeassistant.config_entries import ConfigFlow, OptionsFlow, ConfigEntry
from homeassistant.core import callback
from homeassistant.data_entry_flow import FlowResult

from .const import DOMAIN, EFFECT_LIST, EFFECT_NONE
from . import hidraw

_LOGGER = logging.getLogger(__name__)


class BusyMeConfigFlow(ConfigFlow, domain=DOMAIN):
    """Handle a config flow for Busy Me Light."""

    VERSION = 1

    async def async_step_user(
        self, user_input: dict[str, Any] | None = None
    ) -> FlowResult:
        """Handle the initial step — scan for USB devices."""
        errors = {}

        if user_input is not None:
            devices = await self._discover()
            if devices:
                n = len(devices)
                return self.async_create_entry(
                    title=f"Busy Me ({n} light{'s' if n > 1 else ''})",
                    data={},
                )
            errors["base"] = "no_devices"

        return self.async_show_form(
            step_id="user",
            data_schema=vol.Schema({}),
            errors=errors,
        )

    async def async_step_discovery(
        self, discovery_info: dict[str, Any] | None = None
    ) -> FlowResult:
        return await self.async_step_user()

    @staticmethod
    @callback
    def async_get_options_flow(config_entry: ConfigEntry) -> BusyMeOptionsFlow:
        return BusyMeOptionsFlow(config_entry)

    async def _discover(self) -> list:
        """Run hidraw scan."""
        import asyncio
        loop = asyncio.get_running_loop()
        try:
            return await loop.run_in_executor(None, hidraw.scan)
        except Exception as exc:
            _LOGGER.error("Discovery scan failed: %s", exc)
            return []


class BusyMeOptionsFlow(OptionsFlow):
    """Handle options for a Busy Me Light config entry."""

    def __init__(self, config_entry: ConfigEntry) -> None:
        self._entry = config_entry

    async def async_step_init(
        self, user_input: dict[str, Any] | None = None
    ) -> FlowResult:
        if user_input is not None:
            return self.async_create_entry(title="", data=user_input)

        current = self._entry.options

        schema = vol.Schema(
            {
                vol.Optional(
                    "default_effect",
                    default=current.get("default_effect", EFFECT_NONE),
                ): vol.In(EFFECT_LIST),
                vol.Optional(
                    "poll_interval",
                    default=current.get("poll_interval", 30),
                ): vol.All(vol.Coerce(int), vol.Range(min=5, max=300)),
            }
        )

        return self.async_show_form(
            step_id="init",
            data_schema=schema,
        )

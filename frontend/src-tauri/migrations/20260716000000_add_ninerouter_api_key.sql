-- Migration: Add 9Router API Key to settings table
-- Adds support for the 9Router self-hosted AI router provider

ALTER TABLE settings ADD COLUMN nineRouterApiKey TEXT;

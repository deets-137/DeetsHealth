-- The voice's cost by vendor (docs/architecture/VOICE-VENDORS.md §2.4, §9): a voice call's credits
-- by vendor, worked out by ehr with the voice crate (`cost::credits`) when it files the record.
-- Counts only, never a word of the call. Both schemas (DATABASE.md §7.2).
--
-- `voice_credits`: {"<vendor id>": credits}, e.g. {"cartesia": 812}; {} for a text call. It
-- replaces `cartesia_credits` (migration 0014), whose values move into it as Cartesia's.

ALTER TABLE synthetic.calls ADD COLUMN voice_credits jsonb NOT NULL DEFAULT '{}';
ALTER TABLE real.calls ADD COLUMN voice_credits jsonb NOT NULL DEFAULT '{}';

UPDATE synthetic.calls SET voice_credits = jsonb_build_object('cartesia', cartesia_credits) WHERE cartesia_credits > 0;
UPDATE real.calls SET voice_credits = jsonb_build_object('cartesia', cartesia_credits) WHERE cartesia_credits > 0;

ALTER TABLE synthetic.calls DROP COLUMN cartesia_credits;
ALTER TABLE real.calls DROP COLUMN cartesia_credits;

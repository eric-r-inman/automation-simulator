-- Reverse of 0001_init_schema.up.sql.  Dropped in fkey-dependency
-- order so foreign-key constraints never block the drop.

DROP INDEX IF EXISTS ix_sim_event_by_run;
DROP TABLE IF EXISTS property_design;
DROP TABLE IF EXISTS sim_event;
DROP TABLE IF EXISTS sim_run;
DROP TABLE IF EXISTS schedule_item;
DROP INDEX IF EXISTS ix_watering_log_by_zone_time;
DROP TABLE IF EXISTS watering_log;
DROP INDEX IF EXISTS ix_sensor_reading_by_zone_time;
DROP TABLE IF EXISTS sensor_reading;
DROP TABLE IF EXISTS sensor_instance;
DROP TABLE IF EXISTS controller_instance;
DROP TABLE IF EXISTS plant;
DROP TABLE IF EXISTS zone;
DROP TABLE IF EXISTS manifold;
DROP TABLE IF EXISTS spigot;
DROP TABLE IF EXISTS yard;
DROP TABLE IF EXISTS property;

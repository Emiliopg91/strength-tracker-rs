--- USB device column

ALTER TABLE device ADD COLUMN usb BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE device ADD COLUMN connect BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE device ADD COLUMN strength_workout BOOLEAN NOT NULL DEFAULT false;
UPDATE device SET usb=true, connect=false WHERE serial!='connect';
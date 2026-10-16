import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { BackendClient } from "@/utils/backend/client";
import { SessionLap } from "@/utils/backend/models";
import { SessionUtils } from "@/utils/SessionUtils";
import { useContext, useEffect, useRef, useState } from "react";
import { IconButton, Tooltip } from "@mui/material";
import { MapContainer, Marker, Polyline, TileLayer } from "react-leaflet";
import FileDownloadIcon from "@mui/icons-material/FileDownload";
import MapIcon from "@mui/icons-material/Map";
import SatelliteAltIcon from "@mui/icons-material/SatelliteAlt";
import L from "leaflet";

const urls = [
    "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png",
    "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
];

type Props = {
    timestamp: number;
    validPoints: [number, number][];
    gpsSegments: {
        coordinates: [[number, number], [number, number]];
        color: string;
    }[];
    startPoint: [number, number];
    finishPoint: [number, number];
    laps: SessionLap[];
};

export function SessionMap({
    timestamp,
    validPoints,
    gpsSegments,
    startPoint,
    finishPoint,
    laps,
}: Props) {
    const withLoading = useLoadingTask();
    const { translate } = useContext(I18nSettingsContext);
    const [url, setUrl] = useState(1);
    const exportControlRef = useRef<HTMLDivElement>(null);
    const typeControlRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        for (const control of [exportControlRef, typeControlRef]) {
            if (control.current) {
                L.DomEvent.disableClickPropagation(control.current);
            }
        }
    }, []);

    const toggleMapType = () => {
        setUrl((current) => (current === 0 ? 1 : 0));
    };

    const exportTrack = () => {
        withLoading(BackendClient.exportGpx(timestamp));
    };

    return (
        <MapContainer
            bounds={validPoints}
            boundsOptions={{ padding: [20, 20] }}
            attributionControl={false}
            className="session-map"
        >
            <TileLayer url={urls[url]} attribution={""} />

            {gpsSegments.map((val, idx) => (
                <Polyline
                    key={"segment-" + idx}
                    positions={val.coordinates}
                    color={val.color}
                    weight={4}
                />
            ))}

            <Marker
                position={startPoint}
                icon={SessionUtils.START_ICON}
            ></Marker>

            <Marker
                position={finishPoint}
                icon={SessionUtils.END_ICON}
            ></Marker>

            {laps
                .filter((lap) => lap.start_latitude && lap.start_longitude)
                .map((lap) =>
                    SessionUtils.makeLapMarker(lap.idx + 1, [
                        lap.start_latitude!,
                        lap.start_longitude!,
                    ]),
                )}

            <div className="leaflet-top leaflet-right">
                <div
                    ref={exportControlRef}
                    className="leaflet-control session-map-control"
                >
                    <Tooltip title={translate("export_gpx")}>
                        <IconButton size="small" onClick={exportTrack}>
                            <FileDownloadIcon fontSize="small" />
                        </IconButton>
                    </Tooltip>
                </div>
            </div>

            <div className="leaflet-bottom leaflet-right">
                <div
                    ref={typeControlRef}
                    className="leaflet-control session-map-control"
                >
                    <Tooltip
                        title={translate(
                            url === 0 ? "satellite_map" : "street_map",
                        )}
                    >
                        <IconButton size="small" onClick={toggleMapType}>
                            {url === 0 ? (
                                <SatelliteAltIcon fontSize="small" />
                            ) : (
                                <MapIcon fontSize="small" />
                            )}
                        </IconButton>
                    </Tooltip>
                </div>
            </div>
        </MapContainer>
    );
}

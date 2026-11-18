import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { SessionLapDto } from "@/utils/backend/models";
import { SessionUtils } from "@/utils/SessionUtils";
import { useContext, useEffect, useRef, useState } from "react";
import { IconButton, Tooltip } from "@mui/material";
import MapIcon from "@mui/icons-material/Map";
import SatelliteAltIcon from "@mui/icons-material/SatelliteAlt";
import L from "leaflet";

const urls = [
    "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png",
    "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
];

const BOUNDS_PADDING: L.PointExpression = [20, 20];

type Props = {
    validPoints: [number, number][];
    gpsSegments: {
        coordinates: [[number, number], [number, number]];
        color: string;
    }[];
    startPoint: [number, number];
    finishPoint: [number, number];
    laps: SessionLapDto[];
};

export function SessionMap({
    validPoints,
    gpsSegments,
    startPoint,
    finishPoint,
    laps,
}: Props) {
    const { translate } = useContext(I18nSettingsContext);
    const [url, setUrl] = useState(1);
    const containerRef = useRef<HTMLDivElement>(null);
    const mapRef = useRef<L.Map | null>(null);
    const tilesRef = useRef<L.TileLayer | null>(null);
    // The view is fitted to the route once; later data changes (session edits) keep the user's pan/zoom
    const fittedRef = useRef(false);

    useEffect(() => {
        const map = L.map(containerRef.current!, {
            attributionControl: false,
        });
        mapRef.current = map;
        return () => {
            map.remove();
            mapRef.current = null;
            tilesRef.current = null;
            fittedRef.current = false;
        };
    }, []);

    useEffect(() => {
        const map = mapRef.current!;
        tilesRef.current?.remove();
        tilesRef.current = L.tileLayer(urls[url]).addTo(map);
    }, [url]);

    useEffect(() => {
        const map = mapRef.current!;
        const group = L.layerGroup();

        for (const segment of gpsSegments) {
            L.polyline(segment.coordinates, {
                color: segment.color,
                weight: 4,
            }).addTo(group);
        }
        L.marker(startPoint, { icon: SessionUtils.START_ICON }).addTo(group);
        L.marker(finishPoint, { icon: SessionUtils.END_ICON }).addTo(group);
        for (const lap of laps) {
            if (lap.start_latitude && lap.start_longitude) {
                L.marker([lap.start_latitude, lap.start_longitude], {
                    icon: SessionUtils.makeLapIcon(lap.idx + 1),
                }).addTo(group);
            }
        }
        group.addTo(map);

        if (!fittedRef.current) {
            map.fitBounds(validPoints, { padding: BOUNDS_PADDING });
            fittedRef.current = true;
        }

        return () => {
            group.remove();
        };
    }, [validPoints, gpsSegments, startPoint, finishPoint, laps]);

    const toggleMapType = () => {
        setUrl((current) => (current === 0 ? 1 : 0));
    };

    return (
        <div className="session-map">
            <div ref={containerRef} className="session-map-canvas" />
            <div className="leaflet-bottom leaflet-right">
                <div className="leaflet-control session-map-control">
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
        </div>
    );
}

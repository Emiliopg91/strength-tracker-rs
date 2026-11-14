import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { BackendClient } from "@/utils/backend/client";
import { CloudProvider } from "@/utils/backend/models";
import { useContext, useState } from "react";
import { Button, Menu, MenuItem } from "@mui/material";

export function UploadToCloudMenu() {
    const withLoading = useLoadingTask();
    const { translate } = useContext(I18nSettingsContext);
    const [importMenuAnchor, setImportMenuAnchor] = useState<{
        top: number;
        left: number;
    } | null>(null);

    const uploadToCloud = (provider: CloudProvider) => {
        withLoading(BackendClient.uploadToCloud(provider));
    };

    return (
        <>
            <Button
                variant="contained"
                className="full-width-button"
                onClick={(e) =>
                    setImportMenuAnchor({ top: e.clientY, left: e.clientX })
                }
            >
                {translate("upload_to")}
            </Button>
            <Menu
                id="import-file-menu"
                anchorReference="anchorPosition"
                anchorPosition={importMenuAnchor ?? undefined}
                open={Boolean(importMenuAnchor)}
                onClose={() => setImportMenuAnchor(null)}
                anchorOrigin={{
                    vertical: "top",
                    horizontal: "left",
                }}
                transformOrigin={{
                    vertical: "bottom",
                    horizontal: "left",
                }}
            >
                <MenuItem
                    onClick={() => {
                        setImportMenuAnchor(null);
                        uploadToCloud(CloudProvider.DropBox);
                    }}
                >
                    DropBox
                </MenuItem>
                <MenuItem
                    onClick={() => {
                        setImportMenuAnchor(null);
                        uploadToCloud(CloudProvider.OneDrive);
                    }}
                >
                    OneDrive
                </MenuItem>
            </Menu>
        </>
    );
}

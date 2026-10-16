import { createTheme } from "@mui/material/styles";

export const theme = createTheme({
    palette: {
        mode: "dark",
        primary: {
            main: "#1779bd",
        },
        background: {
            default: "#303030",
            paper: "#101010",
        },
    },
    components: {
        MuiDialog: {
            defaultProps: {},
        },
    },
});

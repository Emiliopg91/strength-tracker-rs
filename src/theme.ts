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
      defaultProps: {
        // The fade-in renders as a see-through flicker in the webview
        transitionDuration: 0,
      },
    },
  },
});

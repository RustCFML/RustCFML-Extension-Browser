component {

    this.name = "browser-testbox-example";

    // TestBox, wherever you keep it. `box install testbox` puts it in
    // ./testbox; this repo's own checkout sits a few levels up.
    local_tb = expandPath( "./testbox" );
    repo_tb  = expandPath( "../../../TestBox" );
    this.mappings[ "/testbox" ] = directoryExists( local_tb ) ? local_tb : repo_tb;
    this.mappings[ "/specs" ]   = expandPath( "./specs" );

}

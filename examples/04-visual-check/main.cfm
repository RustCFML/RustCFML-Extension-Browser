<cfscript>
/*  Visual checks: did the page render, and does it look right at each size?

    Three things this demonstrates:
      1. `detail=true` — catch a blank or half-rendered page automatically.
      2. Screenshots at several viewports, for a responsive-layout eyeball.
      3. A simple visual-regression loop: compare against a stored baseline.
*/

here = getDirectoryFromPath( getCurrentTemplatePath() );
url  = "https://en.wikipedia.org/wiki/Rust_(programming_language)";

page = Browser( { timeout = 60000 } ).newPage().goto( url ).settle( 400 );

// 1. Is there anything on the screen at all? A page whose script threw before
//    painting produces a perfectly valid, perfectly blank PNG.
check = page.screenshot( { width = 1280, height = 800, detail = true } );
writeOutput( "rendered:      " & yesNoFormat( !check.looksUnrendered )
             & "  (ink " & numberFormat( check.inkCoverage * 100, "0.0" ) & "%, " & check.distinctColours & " colours)" & chr(10) );
if ( check.looksUnrendered ) throw( "page painted nothing" );

// 2. Responsive sweep. Same page, three widths, three files.
for ( vp in [ { name = "mobile", w = 390, h = 844 }, { name = "tablet", w = 820, h = 1180 }, { name = "desktop", w = 1440, h = 900 } ] ) {
    png = page.setViewport( vp.w, vp.h ).screenshot( { width = vp.w, height = vp.h } );
    fileWrite( "#here#wiki-#vp.name#.png", png );
    writeOutput( "wiki-#vp.name#.png:  #vp.w#x#vp.h#, " & numberFormat( len( png ) / 1024, "0" ) & " KB" & chr(10) );
}

// 3. Regression against a baseline. The first run stores the baseline; later
//    runs compare the layout of what actually matters (positions of key
//    elements), which is far more stable than comparing pixels of live content.
layout = page.setViewport( 1280, 800 ).evaluate( "
    (function(){
      var pick = ['h1', '.infobox', '.mw-body-content p', '##p-navigation, ##vector-main-menu, nav'];
      var out = {};
      pick.forEach(function(sel){
        var e = document.querySelector(sel); if (!e) return;
        var r = e.getBoundingClientRect();
        out[sel] = { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) };
      });
      return out;
    })()
" );

baselineFile = "#here#baseline.json";
if ( !fileExists( baselineFile ) ) {
    fileWrite( baselineFile, serializeJSON( layout ) );
    fileWrite( "#here#baseline.png", page.screenshot( { width = 1280, height = 800 } ) );
    writeOutput( "baseline:      stored (baseline.json + baseline.png). Run again to compare." & chr(10) );
} else {
    baseline = deserializeJSON( fileRead( baselineFile ) );
    drift = [];
    for ( sel in baseline ) {
        if ( !structKeyExists( layout, sel ) ) { drift.append( "#sel#: missing" ); continue; }
        a = baseline[ sel ]; b = layout[ sel ];
        if ( abs( a.w - b.w ) > 4 || abs( a.h - b.h ) > 40 || abs( a.x - b.x ) > 4 ) {
            drift.append( "#sel#: #a.w#x#a.h#@#a.x# -> #b.w#x#b.h#@#b.x#" );
        }
    }
    if ( arrayLen( drift ) ) {
        fileWrite( "#here#current.png", page.screenshot( { width = 1280, height = 800 } ) );
        writeOutput( "layout drift:  " & arrayLen( drift ) & " element(s) moved — see current.png vs baseline.png" & chr(10) );
        for ( d in drift ) writeOutput( "   " & d & chr(10) );
    } else {
        writeOutput( "layout drift:  none — matches baseline" & chr(10) );
    }
}

page.close();
</cfscript>

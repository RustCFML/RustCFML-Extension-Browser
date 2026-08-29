<cfscript>
/*  Render a CFML-generated HTML report to PDF.

    Build the markup in CFML the way you always have, write it to disk, load
    it, and print it. You get what the page looks like in print media —
    backgrounds, fonts, layout — sliced across pages.

    This is a raster PDF (text is not selectable). Use it for archiving a
    rendered report or visual sign-off; use the Typst extension for a vector
    PDF with searchable text.
*/

here = getDirectoryFromPath( getCurrentTemplatePath() );

// Some data — in real life a query.
rows = [];
regions = [ "North", "South", "East", "West", "Central" ];
for ( m = 1; m <= 12; m++ ) {
    for ( region in regions ) {
        rows.append( { month = monthAsString( m ), region = region, revenue = 10000 + randRange( 0, 25000 ), orders = randRange( 40, 400 ) } );
    }
}
total = 0;
for ( r in rows ) total += r.revenue;

savecontent variable="html" {
    writeOutput( '<!doctype html><html><head><meta charset="utf-8"><title>Revenue report</title>
    <style>
      body{font-family:Georgia,serif;margin:40px;color:##111}
      header{display:flex;justify-content:space-between;align-items:flex-end;border-bottom:3px solid ##1d4ed8;padding-bottom:12px;margin-bottom:24px}
      h1{margin:0;font-size:28px;color:##1d4ed8} .meta{color:##6b7280;font-size:13px}
      .summary{display:flex;gap:16px;margin-bottom:24px}
      .card{flex:1;background:##eff6ff;border-radius:8px;padding:16px} .card b{display:block;font-size:24px;color:##1d4ed8}
      table{width:100%;border-collapse:collapse;font-size:13px}
      th{background:##1d4ed8;color:##fff;text-align:left;padding:8px} td{padding:6px 8px;border-bottom:1px solid ##e5e7eb}
      tr:nth-child(even) td{background:##f9fafb} td.num{text-align:right;font-variant-numeric:tabular-nums}
    </style></head><body>
    <header><h1>Revenue by region</h1><div class="meta">Generated #dateTimeFormat( now(), "d mmm yyyy HH:nn" )#</div></header>
    <div class="summary">
      <div class="card"><b>#numberFormat( total, "," )#</b>total revenue</div>
      <div class="card"><b>#arrayLen( rows )#</b>rows</div>
      <div class="card"><b>#arrayLen( regions )#</b>regions</div>
    </div>
    <table><thead><tr><th>Month</th><th>Region</th><th style="text-align:right">Revenue</th><th style="text-align:right">Orders</th></tr></thead><tbody>' );
    for ( r in rows ) {
        writeOutput( '<tr><td>#r.month#</td><td>#r.region#</td><td class="num">#numberFormat( r.revenue, "," )#</td><td class="num">#r.orders#</td></tr>' );
    }
    writeOutput( '</tbody></table></body></html>' );
}

fileWrite( "#here#report.html", html );

page = Browser().newPage().goto( "file://#here#report.html" );

pdf = page.pdf( { paper = "A4", printBackground = true, margin = 0.5 } );
fileWrite( "#here#report.pdf", pdf );
writeOutput( "report.pdf: " & numberFormat( len( pdf ) / 1024, "0" ) & " KB" & chr(10) );

// Landscape and Letter are a flag away.
fileWrite( "#here#report-landscape.pdf", page.pdf( { paper = "Letter", landscape = true, printBackground = true } ) );
writeOutput( "report-landscape.pdf written" & chr(10) );

// And a full-page PNG of the same document, for a thumbnail or a quick eyeball.
fileWrite( "#here#report.png", page.screenshot( { width = 900, fullPage = true } ) );
writeOutput( "report.png written" & chr(10) );

page.close();
</cfscript>

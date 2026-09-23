const fs = require('fs');

async function main() {
    const layer = JSON.parse(fs.readFileSync('aq_north_raw.json', 'utf8').replace(/^\uFEFF/, ''));
    console.log('Layer keys:', Object.keys(layer));
    console.log('cachedDrawingInfo:', layer.cachedDrawingInfo);
    console.log('drawingInfo:', JSON.stringify(layer.drawingInfo, null, 2));
    console.log('store:', layer.store);
    console.log('geometryDefinitions:', JSON.stringify(layer.geometryDefinitions, null, 2));
    console.log('nodePages:', layer.nodePages);

    // Let's fetch node page 0 if nodePages exists, or fetch root node
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    let nodeUrl = baseUrl + '/nodes/root';
    if (layer.nodePages) {
        nodeUrl = baseUrl + '/nodepages/0';
    }
    const res = await fetch(nodeUrl + '?f=json');
    const nodeData = await res.json();
    console.log('Fetched node data keys:', Object.keys(nodeData));
    if (nodeData.nodes) {
        console.log('Num nodes in page 0:', nodeData.nodes.length);
        console.log('Sample node 0:', JSON.stringify(nodeData.nodes[0], null, 2));
        console.log('Sample node with mesh:', JSON.stringify(nodeData.nodes.find(n => n.mesh), null, 2));
    } else {
        console.log('Root node:', JSON.stringify(nodeData, null, 2));
    }
}

main().catch(console.error);

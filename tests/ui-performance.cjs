const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const source=fs.readFileSync(require('node:path').join(__dirname, '../ui/app.js'),'utf8');
function harness(){
 const calls=[],pending=[],listeners={},timers=[];
 const elements=new Map();
 const element=id=>{if(!elements.has(id)) elements.set(id,{textContent:'',classList:{add(){},remove(){},contains(){return false}},addEventListener(name,fn){this[name]=fn},appendChild(){},querySelectorAll(){return []}});return elements.get(id)};
 const tabs=['accounts','patcher','cleaner','doctor'].map(id=>Object.assign(element('nav-'+id),{dataset:{tab:id},setAttribute(){}}));
 const context=vm.createContext({console,setTimeout(fn,delay){timers.push({fn,delay});},window:{__TAURI__:{core:{invoke(cmd,args){calls.push([cmd,args]);return new Promise((resolve,reject)=>pending.push({cmd,args,resolve,reject}));}}}},document:{createElement(){return {style:{},remove(){}}},querySelectorAll(s){return s==='.nav-tab'?tabs:[]},getElementById:element,addEventListener(n,fn){listeners[n]=fn}},confirm(){return false}});
 vm.runInContext(source,context);
 return {context,calls,pending,listeners,tabs,timers,element};
}
test('startup loads accounts and token maintenance; hidden scans are lazy',()=>{
 const h=harness();h.listeners.DOMContentLoaded();
 assert.deepEqual(h.calls.map(c=>c[0]),['get_accounts','refresh_due_tokens']);
 h.tabs[3].click();h.tabs[3].click();
 assert.deepEqual(h.calls.map(c=>c[0]),['get_accounts','refresh_due_tokens','get_doctor_report']);
});
test('overview/card share one quota request and concurrency is capped at two',async()=>{
 const h=harness();
 const same=vm.runInContext("requestQuota('a') === requestQuota('a')",h.context);
 assert.equal(same,true);
 vm.runInContext("requestQuota('b'); requestQuota('c');",h.context);
 assert.equal(h.calls.length,2);
 h.pending[0].resolve({});
 await new Promise(resolve=>setImmediate(resolve));
 assert.equal(h.calls.length,3);
 assert.equal(h.calls[2][1].name,'c');
});
test('failed quota requests can be retried',async()=>{
 const h=harness();vm.runInContext("requestQuota('a').catch(()=>{});",h.context);
 h.pending[0].reject(new Error('offline'));
 await new Promise(resolve=>setImmediate(resolve));
 vm.runInContext("requestQuota('a');",h.context);
 assert.equal(h.calls.length,2);
});

test('token refresh waits for completion and backs off after failure',async()=>{
 const h=harness();
 const finished=vm.runInContext('startTokenRefresh()',h.context);
 assert.equal(h.calls[0][0], 'refresh_due_tokens');
 assert.equal(h.timers.length,0);
 h.pending[0].resolve({updated:[],failed:['test-profile']});
 await finished;
 assert.equal(h.timers.at(-1).delay,300000);
});
test('successful token check schedules the next check in one minute',async()=>{
 const h=harness();
 const finished=vm.runInContext('startTokenRefresh()',h.context);
 h.pending[0].resolve({updated:[],failed:[]});
 await finished;
 assert.equal(h.timers.at(-1).delay,60000);
});

test('updater checks without installing, then installs and restarts on separate clicks',async()=>{
 const h=harness();
 const check=vm.runInContext('runUpdater()',h.context);
 assert.equal(h.calls[0][0],'check_update');
 assert.equal(h.element('btn-check-update').disabled,true);
 h.pending[0].resolve({current_version:'1.4.0',latest_version:'v1.5.0',message:'New version',installable:true,installed:false});
 await check;
 assert.equal(h.calls.length,1);
 assert.equal(h.element('btn-check-update').textContent,'Update to v1.5.0');
 const install=vm.runInContext('runUpdater()',h.context);
 assert.equal(h.calls[1][0],'install_update');
 await vm.runInContext('runUpdater()',h.context); // Ignore a double click.
 assert.equal(h.calls.length,2);
 h.pending[1].resolve('v1.5.0');await install;
 assert.equal(h.element('btn-check-update').textContent,'Restart agx');
 const restart=vm.runInContext('runUpdater()',h.context);
 assert.equal(h.calls[2][0],'restart_after_update');
 h.pending[2].resolve();await restart;
});
test('failed update leaves a retry action and does not restart',async()=>{
 const h=harness();
 vm.runInContext("updaterState='install'",h.context);
 const install=vm.runInContext('runUpdater()',h.context);
 h.pending[0].reject(new Error('SHA-256 mismatch'));await install;
 assert.equal(h.element('btn-check-update').disabled,false);
 assert.equal(h.element('btn-check-update').textContent,'Retry update');
 assert.equal(h.calls.length,1);
 assert.match(h.element('update-status').textContent,/SHA-256 mismatch/);
});

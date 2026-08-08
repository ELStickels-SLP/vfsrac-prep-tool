hopRatio = 0.5; % scalar % 80% fewer output samples than input samples
WindowLen = 512; % scalar
AnalysisLen = 220; % scalar
SynthesisLen = round(AnalysisLen*hopRatio); % scalar
Hopratio = SynthesisLen/AnalysisLen; % scalar
Fs = 44100; % scalar
reader = dsp.AudioFileReader('AudioSample.wav','SamplesPerFrame',AnalysisLen,... % object (System object)
'OutputDataType','double');
readerInfo = audioinfo("AudioSample.wav"); % struct (object)
% reader = audioDeviceReader(Fs,AnalysisLen)
win = sqrt(hanning(WindowLen,'periodic')); % vector
stft = dsp.STFT(win, WindowLen - AnalysisLen, WindowLen); % object (System object)
istft = dsp.ISTFT(win, WindowLen - SynthesisLen ); % object (System object)
player = audioDeviceWriter('SampleRate',Fs, ... % object (System object)
'SupportVariableSizeInput',true, ...
'BufferSize',512);
setup(player,zeros(reader.SamplesPerFrame,readerInfo.NumChannels)); % zeros(...) here: matrix (samples x channels)
unwrapdata = 2*pi*AnalysisLen*(0:WindowLen-1)'/WindowLen; % vector
yangle = zeros(WindowLen,1); % vector
firsttime = true; % scalar (boolean)
logger = dsp.SignalSink(); % object (System object)
while ~isDone(reader)
    % y = reader();
    audioFromDevice = reader(); % matrix (samples x channels)
    y = audioFromDevice(:,1); % vector one channel
    % y(abs(y)<1e-2)=0;
    % ST-FFT
    yfft = stft(y); % vector (complex)
    % Convert complex FFT data to magnitude and phase.
    ymag = abs(yfft); % vector
    yprevangle = yangle; % vector
    yangle = angle(yfft); % vector
    % Synthesis Phase Calculation
    % The synthesis phase is calculated by computing the phase increments
    % between successive frequency transforms, unwrapping them, and scaling
    % them by the ratio between the analysis and synthesis hop sizes.
    yunwrap = (yangle - yprevangle) - unwrapdata; % vector
    yunwrap = yunwrap - round(yunwrap/(2*pi))*2*pi; % vector
    yunwrap = (yunwrap + unwrapdata) * Hopratio; % vector
    if firsttime
        ysangle = yangle; % vector
        firsttime = false; % scalar (boolean)
    else
        ysangle = ysangle + yunwrap; % vector
    enda
    % Convert magnitude and phase to complex numbers.
    ys = ymag .* complex(cos(ysangle), sin(ysangle)); % vector (comple`x)
    % IST-FFT
    yistfft = istft(ys); % vector
    yout = interp1((1:SynthesisLen),yistfft,linspace(1,SynthesisLen,AnalysisLen))'; % vector
    player(yout);
    logger(yout);
end
yshifted = logger. Buffer(200:end)'; % vector
%{
==================================================================================

The function process_window() should have the following operational steps

      Invoked when “analysis_len” samples are available from the audio input device
      prev_phase is 512 values set to zero prior to first time called
      phase_unwrap = 2*pi*analysis_len*(0->511)/512  : used to unwrap the spectrum phase values
Step 1:  Place the analysis_len new samples at the head of the circular 512 sample FFT buffer, keeping the previous (512-analysis_len) samples at the tail of the buffer

Step 2:  perform forward FFT with circular buffer data

Step 3:  find the fundamental voice tone (FFT bin with peak after DC bin)

Step 4:  Compute the compression ratio (expansion)  to cause the tone to move up the desired up-shift
      The hopRatio = (Fpeak+Fshift)/Fpeak    - requested hopRation to achieve Fshift of fundamental
      The synthesis_len = ceil(hopRatio*analysis_len)
      hopRatio = synthesis_len/analysis_len.  - actual hop ratio achieved (close to requested)

Step 5:  Synthesize enough output samples that you can compress the time-series to effect the up-shift
         compute magnitude and phase of the FFT spectrum
          mag = sqrt(real(X)^2+imag(x)^2)
          phase = arctan(imag(x)/real(x)
      for first time invoked, just set out_phase to phase
      for calls after the first time, then
      unwrap the phase series:  out_phase =  phase -previous_phase - phase_unwrap
      Fix the phase vector to nearest 2*pi cycle boundary:  out_phase = fix(out_phase/2/pi)*2*pi
      Now set the phase for the FFT buffer to account for the added time output
      out_phase = out_phase + unwrap_phase

Step 6:  Prepare buffer for inverse FFT
      prev_phase = phase
      ifft_buffer = mag * (cos(out_phase)+I*sin(out_phase))
 
Step 7:
      inverse FFT the ifft_buffer to generate 512 real time samples (out_buf)
Step 8:
      resample the out_buf to  analysis_len samples that span synthesis_len samples
      alpha = (n*hopRatio-fix(n*hopRation)
      out_buf(n) = alpha*out_buf(n-1)+(1-alpha)*out_buf(n)
Step 9:
      send out_buf to audio output device (analysis_len samples in out_buf)

Ready for next input   
%}
